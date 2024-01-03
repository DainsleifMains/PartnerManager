use crate::database::get_database_connection;
use crate::models::Partner;
use crate::schema::partners;
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use miette::{bail, IntoDiagnostic};
use serenity::builder::{
	CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu,
	CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse,
};
use serenity::client::Context;
use serenity::collector::ComponentInteractionCollector;
use serenity::model::application::{
	ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionDataKind,
};
use std::time::Duration;

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
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

	if partners.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("You have no partners to remove.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

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
		.label("Remove")
		.style(ButtonStyle::Danger);
	let cancel_button = CreateButton::new(&cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let partner_row = CreateActionRow::SelectMenu(partner_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Select the partner to remove")
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
			.timeout(Duration::from_secs(45))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("No partner was removed.")
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
						.content("No partner was removed.");
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Message(message))
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected interaction type received by partners remove command: {:?}",
				interaction.data.kind
			),
		}
	};

	if partner_id.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No partner was removed.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let partner_display_name = partners
		.iter()
		.find(|partner| partner.partnership_id == partner_id)
		.map(|partner| partner.display_name.clone());
	let Some(partner_display_name) = partner_display_name else {
		bail!("Partner selection desynchronized with partner list");
	};

	let mut db_connection = db_connection.lock().await;
	diesel::delete(partners::table)
		.filter(partners::partnership_id.eq(&partner_id))
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	// TODO update embed
	// TODO role sync

	let message =
		CreateInteractionResponseMessage::new().content(format!("Removed {} as a partner.", partner_display_name));
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
