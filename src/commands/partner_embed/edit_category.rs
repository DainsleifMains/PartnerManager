use crate::database::get_database_connection;
use crate::models::{EmbedData, PartnerCategory};
use crate::schema::{embed_data, partner_categories};
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
		bail!("Partner embed command was run outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let (embeds, categories) = {
		let mut db_connection = db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
			return Ok(());
		}

		let embeds: Vec<EmbedData> = embed_data::table
			.filter(embed_data::guild.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?;
		let categories: Vec<PartnerCategory> = partner_categories::table
			.filter(partner_categories::guild_id.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?;

		(embeds, categories)
	};

	if embeds.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("There are no embeds to edit.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	if categories.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("You have no partner categories to add to messages.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let embed_select_options: Vec<CreateSelectMenuOption> = embeds
		.iter()
		.map(|embed| CreateSelectMenuOption::new(&embed.embed_name, &embed.id))
		.collect();
	let category_select_options: Vec<CreateSelectMenuOption> = categories
		.iter()
		.map(|category| CreateSelectMenuOption::new(&category.name, &category.id))
		.collect();

	let embed_select_id = cuid2::create_id();
	let category_select_id = cuid2::create_id();
	let submit_button_id = cuid2::create_id();
	let cancel_button_id = cuid2::create_id();

	let embed_select = CreateSelectMenu::new(
		&embed_select_id,
		CreateSelectMenuKind::String {
			options: embed_select_options,
		},
	)
	.placeholder("Embed");
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

	let embed_row = CreateActionRow::SelectMenu(embed_select);
	let category_row = CreateActionRow::SelectMenu(category_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new().ephemeral(true).content("Select the embed to modify and the partner category list to have it use. If you want it to stop displaying a partner list, leave the partner category blank.").components(vec![embed_row, category_row, buttons_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut embed_id = String::new();
	let mut category_id = String::new();

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				embed_select_id.clone(),
				category_select_id.clone(),
				submit_button_id.clone(),
				cancel_button_id.clone(),
			])
			.timeout(Duration::from_secs(60))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("No embed was modified.")
				.components(Vec::new());
			command.edit_response(&ctx.http, message).await.into_diagnostic()?;
			return Ok(());
		};
		match &interaction.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap_or_default();
				if interaction.data.custom_id == embed_select_id {
					embed_id = value;
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
						.content("No embed was modified.");
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Message(message))
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected interaction type received by the patner_embed edit_category command: {:?}",
				interaction.data.kind
			),
		}
	};

	if embed_id.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No embed was modified; an embed was not selected.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let Some(embed) = embeds.iter().find(|embed| embed.id == embed_id) else {
		bail!("Embed selections desynchronized with embed list");
	};
	let (category, category_id) = if category_id.is_empty() {
		(None, None)
	} else {
		match categories.iter().find(|category| category.id == category_id) {
			Some(category) => (Some(category), Some(category_id)),
			None => bail!("Partner category selections desynchronized with partner category list"),
		}
	};

	let mut db_connection = db_connection.lock().await;
	diesel::update(embed_data::table)
		.filter(embed_data::id.eq(&embed.id))
		.set(embed_data::partner_category_list.eq(&category_id))
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	let message_content = match category {
		Some(category) => format!(
			"The embed {} was updated to display the category {}.",
			embed.embed_name, category.name
		),
		None => String::from("The embed {} was updated to remove the partner category display."),
	};
	let message = CreateInteractionResponseMessage::new().content(message_content);
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
