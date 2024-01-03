use crate::database::get_database_connection;
use crate::models::Partner;
use crate::schema::partners;
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
use std::time::Duration;

pub async fn execute(
	ctx: &Context,
	command: &CommandInteraction,
	options: &[ResolvedOption<'_>],
) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Partners command used outside of a guild");
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
			.load(&mut *db_connection)
			.into_diagnostic()?
	};

	let Some(name_option) = options.first() else {
		bail!("Not enough options passed to partners set_name command");
	};
	ensure!(
		name_option.name == "new_display_name",
		severity = Severity::Error,
		"Wrong option passed to partners set_name command: {:?}",
		name_option
	);
	let ResolvedValue::String(new_name) = name_option.value else {
		bail!(
			"Incorrect type provided for new_display_name option: {:?}",
			name_option.value
		);
	};

	let partner_select_options: Vec<CreateSelectMenuOption> = partners
		.iter()
		.map(|partner| CreateSelectMenuOption::new(&partner.display_name, &partner.partnership_id))
		.collect();

	let partner_select_id = cuid2::create_id();
	let submit_button_id = cuid2::create_id();
	let cancel_button_id = cuid2::create_id();

	let partner_select = CreateSelectMenu::new(
		&partner_select_id,
		CreateSelectMenuKind::String {
			options: partner_select_options,
		},
	)
	.placeholder("Partner");
	let submit_button = CreateButton::new(&submit_button_id)
		.label("Update")
		.style(ButtonStyle::Primary);
	let cancel_button = CreateButton::new(&cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let partner_row = CreateActionRow::SelectMenu(partner_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Select the partner to set the name:")
		.components(vec![partner_row, buttons_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut partner_id = String::new();

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				partner_select_id.clone(),
				submit_button_id.clone(),
				cancel_button_id.clone(),
			])
			.timeout(Duration::from_secs(30))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("No display name was updated.")
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
			ComponentInteractionDataKind::Button => {
				if interaction.data.custom_id == submit_button_id {
					break interaction;
				}
				if interaction.data.custom_id == cancel_button_id {
					let message = CreateInteractionResponseMessage::new()
						.ephemeral(true)
						.content("No display name was updated.");
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Message(message))
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected interaction type encountered with partners set_name command: {:?}",
				interaction.data.kind
			),
		}
	};

	if partner_id.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No display name was updated; the partner to update was not selected.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let mut db_connection = db_connection.lock().await;
	let partner_update_result: QueryResult<Partner> = diesel::update(partners::table)
		.filter(partners::partnership_id.eq(&partner_id))
		.set(partners::display_name.eq(&new_name))
		.get_result(&mut *db_connection);

	let message = match partner_update_result {
		Ok(partner) => CreateInteractionResponseMessage::new()
			.content(format!("Updated partner name to {}.", partner.display_name)),
		Err(DbError::NotFound) => CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("That server is no longer a partner."),
		Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content(format!("You already have another partner named {}.", new_name)),
		Err(error) => bail!(error),
	};
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
