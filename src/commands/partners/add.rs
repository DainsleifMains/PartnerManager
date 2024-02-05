use crate::database::get_database_connection;
use crate::models::{Partner, PartnerCategory};
use crate::schema::{partner_categories, partners};
use crate::sync::embed::update_embed;
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DbError};
use miette::{bail, ensure, IntoDiagnostic, Severity};
use serenity::builder::{
	CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu,
	CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse,
};
use serenity::client::Context;
use serenity::collector::ComponentInteractionCollector;
use serenity::model::application::{
	ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionDataKind, ResolvedOption, ResolvedValue,
};
use serenity::model::invite::Invite;
use serenity::utils::parse_invite;
use std::time::Duration;

pub async fn execute(
	ctx: &Context,
	command: &CommandInteraction,
	options: &[ResolvedOption<'_>],
) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Settings command was used outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let partner_categories: Vec<PartnerCategory> = {
		let mut db_connection = db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
			return Ok(());
		}

		partner_categories::table
			.filter(partner_categories::guild_id.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?
	};

	if partner_categories.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("You have no categories to which to add this partner; see `/partner_categories` to create them.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	ensure!(
		!options.is_empty(),
		severity = Severity::Error,
		"not enough options passed to partners add command"
	);

	let mut invite_link = "";
	let mut display_name = String::new();

	for option in options.iter() {
		let ResolvedValue::String(str_value) = option.value else {
			bail!("Non-string value passed to a string option");
		};
		match option.name {
			"invite_link" => invite_link = str_value,
			"display_name" => display_name = str_value.to_string(),
			_ => bail!("Invalid option passed to partners add command: {}", option.name),
		}
	}
	ensure!(
		!invite_link.is_empty(),
		severity = Severity::Error,
		"Not all required options passed to partners add command"
	);

	let invite_code = parse_invite(invite_link);

	// Sometimes, when parsing the invite code, it can maintain an initial slash before the actual code.
	// Somehow, this doesn't seem to break anything in Serenity, and Discord seems to accept it just fine (or Serenity
	// removes the slash), but we want not to have it.
	let invite_code = match invite_code.strip_prefix('/') {
		Some(code) => code,
		None => invite_code,
	};

	let invite = match Invite::get(ctx, invite_code, false, true, None).await {
		Ok(invite) => invite,
		Err(_) => {
			let message = CreateInteractionResponseMessage::new()
				.ephemeral(true)
				.content("The invite link is invalid.");
			command
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
			return Ok(());
		}
	};

	let partner_guild = match invite.guild {
		Some(invite_guild) => invite_guild,
		None => {
			let message = CreateInteractionResponseMessage::new()
				.ephemeral(true)
				.content("The invite link is invalid; could not retrieve a server for it.");
			command
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
			return Ok(());
		}
	};

	if invite.expires_at.is_some() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("The invite link is not permanent.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let category_select_id = cuid2::create_id();
	let submit_button_id = cuid2::create_id();
	let cancel_button_id = cuid2::create_id();

	let category_select_options: Vec<CreateSelectMenuOption> = partner_categories
		.iter()
		.map(|category| CreateSelectMenuOption::new(&category.name, &category.id))
		.collect();
	let category_select = CreateSelectMenu::new(
		&category_select_id,
		CreateSelectMenuKind::String {
			options: category_select_options,
		},
	)
	.placeholder("Partner Category");

	let submit_button = CreateButton::new(&submit_button_id)
		.label("Submit")
		.style(ButtonStyle::Primary);
	let cancel_button = CreateButton::new(&cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let category_row = CreateActionRow::SelectMenu(category_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Choose a category to which to add this partner:")
		.components(vec![category_row, buttons_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut partner_category = String::new();

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				category_select_id.clone(),
				submit_button_id.clone(),
				cancel_button_id.clone(),
			])
			.timeout(Duration::from_secs(30))
			.await
		else {
			let new_message = EditInteractionResponse::new()
				.content("Selection timed out; partner was not added.")
				.components(Vec::new());
			command.edit_response(&ctx.http, new_message).await.into_diagnostic()?;
			return Ok(());
		};
		match &interaction.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap_or_default();
				if interaction.data.custom_id == category_select_id {
					partner_category = value;
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
					let new_message = CreateInteractionResponseMessage::new()
						.ephemeral(true)
						.content("Canceled adding the new partner.");
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Message(new_message))
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected interaction type from components: {:?}",
				interaction.data.kind
			),
		}
	};

	if partner_category.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("Failed to add partner; must select a partner category.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	if display_name.is_empty() {
		display_name = partner_guild.name;
	}
	let partner_guild = partner_guild.id;

	let insert_result = {
		let mut db_connection = db_connection.lock().await;
		let selected_category: Option<PartnerCategory> = partner_categories::table
			.filter(
				partner_categories::id
					.eq(&partner_category)
					.and(partner_categories::guild_id.eq(sql_guild_id)),
			)
			.first(&mut *db_connection)
			.optional()
			.into_diagnostic()?;
		if selected_category.is_none() {
			let message = CreateInteractionResponseMessage::new()
				.ephemeral(true)
				.content("The category you selected is no longer valid.");
			interaction
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
			return Ok(());
		}

		let new_partner = Partner {
			partnership_id: cuid2::create_id(),
			guild: sql_guild_id,
			category: partner_category,
			partner_guild: partner_guild.get() as i64,
			display_name: display_name.clone(),
			invite_code: invite_code.to_string(),
		};
		let insert_result: QueryResult<_> = diesel::insert_into(partners::table)
			.values(new_partner)
			.execute(&mut *db_connection);
		insert_result
	};

	let message = match insert_result {
		Ok(_) => CreateInteractionResponseMessage::new().content(format!(
			"Added [{}](https://discord.gg/{}) as a partner!",
			display_name, invite_code
		)),
		Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, violation_info)) => {
			let message = match violation_info.constraint_name() {
				Some("unique_partner_guild") => "That server is already a partner.",
				Some("unique_partner_display_name") => "That display name is already in use for another partner.",
				_ => "An unknown collision with another partnership occurred.",
			};
			CreateInteractionResponseMessage::new().ephemeral(true).content(message)
		}
		Err(error) => bail!(error),
	};
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	update_embed(ctx, guild).await?;

	Ok(())
}
