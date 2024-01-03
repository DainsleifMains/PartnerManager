use crate::database::get_database_connection;
use crate::models::{Partner, PartnerCategory};
use crate::schema::{partner_categories, partners};
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
		bail!("Partners command was used outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let (partners, partner_categories) = {
		let mut db_connection = db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
			return Ok(());
		}

		let partners: Vec<Partner> = partners::table
			.filter(partners::guild.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?;
		let partner_categories: Vec<PartnerCategory> = partner_categories::table
			.filter(partner_categories::guild_id.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?;

		(partners, partner_categories)
	};

	let partner_select_options: Vec<CreateSelectMenuOption> = partners
		.iter()
		.map(|partner| CreateSelectMenuOption::new(&partner.display_name, &partner.partnership_id))
		.collect();
	let category_select_options: Vec<CreateSelectMenuOption> = partner_categories
		.iter()
		.map(|category| CreateSelectMenuOption::new(&category.name, &category.id))
		.collect();

	let partner_select_id = cuid2::create_id();
	let category_select_id = cuid2::create_id();
	let submit_button_id = cuid2::create_id();
	let cancel_button_id = cuid2::create_id();

	let partner_select = CreateSelectMenu::new(
		&partner_select_id,
		CreateSelectMenuKind::String {
			options: partner_select_options,
		},
	)
	.placeholder("Partner");
	let category_select = CreateSelectMenu::new(
		&category_select_id,
		CreateSelectMenuKind::String {
			options: category_select_options,
		},
	)
	.placeholder("Category");
	let submit_button = CreateButton::new(&submit_button_id)
		.label("Update")
		.style(ButtonStyle::Primary);
	let cancel_button = CreateButton::new(&cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let partner_row = CreateActionRow::SelectMenu(partner_select);
	let category_row = CreateActionRow::SelectMenu(category_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Select the partner and the category that partner should be in.")
		.components(vec![partner_row, category_row, buttons_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut partner_id = String::new();
	let mut category_id = String::new();

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				partner_select_id.clone(),
				category_select_id.clone(),
				submit_button_id.clone(),
				cancel_button_id.clone(),
			])
			.timeout(Duration::from_secs(30))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("No categories were changed.")
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
				} else if interaction.data.custom_id == category_select_id {
					category_id = value;
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
						.content("No categories were changed.");
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Message(message))
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected interaction type encounted by partner set_category command: {:?}",
				interaction.data.kind
			),
		}
	};

	if partner_id.is_empty() || category_id.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No categories were changed.");
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
		bail!("Partner selection desynchronized from partner list");
	};

	let category_name = partner_categories
		.iter()
		.find(|category| category.id == category_id)
		.map(|category| category.name.clone());
	let Some(category_name) = category_name else {
		bail!("Partner category selection desynchronized from partner category list");
	};

	let mut db_connection = db_connection.lock().await;
	diesel::update(partners::table)
		.filter(partners::partnership_id.eq(&partner_id))
		.set(partners::category.eq(&category_id))
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	let message = CreateInteractionResponseMessage::new().content(format!(
		"Updated the category of {} to {}.",
		partner_display_name, category_name
	));
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
