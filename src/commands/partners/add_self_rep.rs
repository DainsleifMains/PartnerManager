use crate::database::get_database_connection;
use crate::models::{Partner, PartnerSelfUser};
use crate::schema::{partner_self_users, partners};
use crate::utils::pagination::{get_partners_for_page, max_partner_page};
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DbError};
use miette::{bail, IntoDiagnostic};
use serenity::builder::{
	CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu,
	CreateSelectMenuKind, EditInteractionResponse,
};
use serenity::client::Context;
use serenity::collector::ComponentInteractionCollector;
use serenity::model::application::{
	ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionDataKind,
};
use serenity::model::id::UserId;
use std::time::Duration;

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Partners command was used outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;

	let partners: Vec<Partner> = {
		let mut db_connection = db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
			return Ok(());
		}

		partners::table
			.filter(partners::guild.eq(sql_guild_id))
			.order(partners::display_name.asc())
			.load(&mut *db_connection)
			.into_diagnostic()?
	};

	if partners.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("You have no partners to which to add representatives.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let mut current_partner_page = 0;
	let partner_select_options = get_partners_for_page(&partners, current_partner_page, "");

	let partner_select_id = cuid2::create_id();
	let user_select_id = cuid2::create_id();
	let submit_id = cuid2::create_id();
	let cancel_id = cuid2::create_id();

	let partner_select = CreateSelectMenu::new(
		&partner_select_id,
		CreateSelectMenuKind::String {
			options: partner_select_options,
		},
	)
	.placeholder("Partner");
	let user_select = CreateSelectMenu::new(&user_select_id, CreateSelectMenuKind::User { default_users: None })
		.placeholder("Our representative");
	let submit_button = CreateButton::new(&submit_id)
		.label("Submit")
		.style(ButtonStyle::Primary);
	let cancel_button = CreateButton::new(&cancel_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let partner_row = CreateActionRow::SelectMenu(partner_select);
	let user_row = CreateActionRow::SelectMenu(user_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Select a partner and the user we're sending to represent them.")
		.components(vec![partner_row, user_row.clone(), buttons_row.clone()]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut partner = String::new();
	let mut user: Option<UserId> = None;

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				partner_select_id.clone(),
				user_select_id.clone(),
				submit_id.clone(),
				cancel_id.clone(),
			])
			.timeout(Duration::from_secs(60))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("No representative was added.")
				.components(Vec::new());
			command.edit_response(&ctx.http, message).await.into_diagnostic()?;
			return Ok(());
		};
		match &interaction.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap();
				if interaction.data.custom_id == partner_select_id {
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;

					if value == "<" {
						current_partner_page = current_partner_page.saturating_sub(0);
					} else if value == ">" {
						current_partner_page = (current_partner_page + 1).min(max_partner_page(&partners));
					} else {
						partner = value;
						continue;
					}

					let partner_select_options = get_partners_for_page(&partners, current_partner_page, &partner);
					let partner_select = CreateSelectMenu::new(
						&partner_select_id,
						CreateSelectMenuKind::String {
							options: partner_select_options,
						},
					);
					let partner_row = CreateActionRow::SelectMenu(partner_select);

					let message = EditInteractionResponse::new().components(vec![
						partner_row,
						user_row.clone(),
						buttons_row.clone(),
					]);
					command.edit_response(&ctx.http, message).await.into_diagnostic()?;
				}
			}
			ComponentInteractionDataKind::UserSelect { values } => {
				let value = values.first().copied().unwrap();
				if interaction.data.custom_id == user_select_id {
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;
					user = Some(value);
				}
			}
			ComponentInteractionDataKind::Button => {
				if interaction.data.custom_id == submit_id {
					break interaction;
				}
				if interaction.data.custom_id == cancel_id {
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
			_ => bail!("Unexpected component interaction type received for partners add_self_rep command"),
		}
	};

	let Some(user) = user else {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No user was added as a representative.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	};
	if partner.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No user was added as a representative.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let Some(partner) = partners
		.iter()
		.find(|check_partner| check_partner.partnership_id == partner)
	else {
		bail!("Partner selections became desynchronized with the partner list");
	};

	let mut db_connection = db_connection.lock().await;
	let new_value = PartnerSelfUser {
		partnership: partner.partnership_id.clone(),
		user_id: user.get() as i64,
	};
	let insert_result = diesel::insert_into(partner_self_users::table)
		.values(new_value)
		.execute(&mut *db_connection);

	match insert_result {
		Ok(_) => {
			let message = CreateInteractionResponseMessage::new().content(format!(
				"Added <@{}> as our representative for {}.",
				user.get(),
				partner.display_name
			));
			interaction
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
		}
		Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			let message = CreateInteractionResponseMessage::new().ephemeral(true).content(format!(
				"<@{}> is already your representative for {}",
				user.get(),
				partner.display_name
			));
			interaction
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
		}
		Err(error) => bail!(error),
	}

	Ok(())
}
