use crate::database::get_database_connection;
use crate::models::{Partner, PartnerUser};
use crate::schema::{guild_settings, partner_users, partners};
use crate::utils::setup_check::GUILD_NOT_SET_UP;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DbError};
use miette::{bail, IntoDiagnostic};
use serenity::builder::{
	CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu,
	CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse,
};
use serenity::client::Context;
use serenity::collector::ComponentInteractionCollector;
use serenity::http::{ErrorResponse, HttpError, StatusCode};
use serenity::model::application::{
	ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionDataKind,
};
use serenity::model::guild::Guild;
use serenity::model::id::UserId;
use serenity::prelude::SerenityError;
use std::time::Duration;

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Partners command was used outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let (partners, partner_role_id) = {
		let mut db_connection = db_connection.lock().await;
		let role_id: Option<Option<i64>> = guild_settings::table
			.find(sql_guild_id)
			.select(guild_settings::partner_role)
			.first(&mut *db_connection)
			.optional()
			.into_diagnostic()?;
		let role_id = match role_id {
			Some(possible_role_id) => possible_role_id,
			None => {
				let message = CreateInteractionResponseMessage::new()
					.ephemeral(true)
					.content(GUILD_NOT_SET_UP);
				command
					.create_response(&ctx.http, CreateInteractionResponse::Message(message))
					.await
					.into_diagnostic()?;
				return Ok(());
			}
		};

		let partners: Vec<Partner> = partners::table
			.filter(partners::guild.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?;

		(partners, role_id.map(|id| id as u64))
	};

	let partner_select_id = cuid2::create_id();
	let representative_select_id = cuid2::create_id();
	let submit_button_id = cuid2::create_id();
	let cancel_button_id = cuid2::create_id();

	let partner_select_options: Vec<CreateSelectMenuOption> = partners
		.iter()
		.map(|partner| CreateSelectMenuOption::new(&partner.display_name, &partner.partnership_id))
		.collect();
	let partner_select = CreateSelectMenu::new(
		&partner_select_id,
		CreateSelectMenuKind::String {
			options: partner_select_options,
		},
	);

	let representative_select = CreateSelectMenu::new(
		&representative_select_id,
		CreateSelectMenuKind::User { default_users: None },
	);

	let submit_button = CreateButton::new(&submit_button_id)
		.label("Submit")
		.style(ButtonStyle::Primary);
	let cancel_button = CreateButton::new(&cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let partner_row = CreateActionRow::SelectMenu(partner_select);
	let representative_row = CreateActionRow::SelectMenu(representative_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Choose the partner server to which to add the representative and the user who is representing them:")
		.components(vec![partner_row, representative_row, buttons_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut partner_id = String::new();
	let mut representative_user: Option<UserId> = None;

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				partner_select_id.clone(),
				representative_select_id.clone(),
				submit_button_id.clone(),
				cancel_button_id.clone(),
			])
			.timeout(Duration::from_secs(120))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("Selection timed out; no partner representative was added.")
				.components(Vec::new());
			command.edit_response(&ctx.http, message).await.into_diagnostic()?;
			return Ok(());
		};
		match &interaction.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap_or_default();
				if interaction.data.custom_id == partner_select_id {
					partner_id = value;
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;
				}
			}
			ComponentInteractionDataKind::UserSelect { values } => {
				let value = values.first().copied();
				if interaction.data.custom_id == representative_select_id {
					representative_user = value;
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;
				}
			}
			ComponentInteractionDataKind::Button => {
				if interaction.data.custom_id == submit_button_id {
					break interaction;
				}
				if interaction.data.custom_id == cancel_button_id {
					let message = CreateInteractionResponseMessage::new()
						.ephemeral(true)
						.content("Canceled adding a representative.");
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Message(message))
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected interaction type for partners add_rep command: {:?}",
				interaction.data.kind
			),
		}
	};

	if partner_id.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("The partner was not selected.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}
	let Some(representative_user) = representative_user else {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("The representative user was not selected.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	};

	let mut db_connection = db_connection.lock().await;
	let partner_data: Option<Partner> = partners::table
		.filter(
			partners::partnership_id
				.eq(&partner_id)
				.and(partners::guild.eq(sql_guild_id)),
		)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let Some(partner_data) = partner_data else {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("The selected partner is not valid.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	};

	let new_partner_user = PartnerUser {
		partnership_id: partner_id,
		user_id: representative_user.get() as i64,
	};
	let insert_result = diesel::insert_into(partner_users::table)
		.values(new_partner_user)
		.execute(&mut *db_connection);
	match insert_result {
		Ok(_) => (),
		Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			let message = CreateInteractionResponseMessage::new().ephemeral(true).content(format!(
				"<@{}> is already a representative for {}.",
				representative_user.get(),
				partner_data.display_name
			));
			interaction
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
			return Ok(());
		}
		Err(error) => bail!(error),
	}

	let mut complain_about_role_permissions = false;
	if let Some(partner_role_id) = partner_role_id {
		let guild_data = Guild::get(&ctx.http, guild).await.into_diagnostic()?;
		let member = guild_data
			.member(&ctx.http, representative_user)
			.await
			.into_diagnostic()?;
		if !member.roles.iter().any(|role| role.get() == partner_role_id) {
			let add_role_result = member.add_role(&ctx.http, partner_role_id).await;
			if let Err(SerenityError::Http(HttpError::UnsuccessfulRequest(ErrorResponse {
				status_code: StatusCode::FORBIDDEN,
				..
			}))) = &add_role_result
			{
				complain_about_role_permissions = true;
			} else {
				add_role_result.into_diagnostic()?
			}
		}
	}

	let mut message_content = format!(
		"Added <@{}> as a partner for {}.",
		representative_user.get(),
		partner_data.display_name
	);
	if complain_about_role_permissions {
		message_content = format!("{}\n**The bot does not have the correct permissions to update partner roles. You will need to add the partner role manually.**", message_content);
	}
	let message = CreateInteractionResponseMessage::new().content(message_content);
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
