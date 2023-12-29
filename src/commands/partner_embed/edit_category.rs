use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::{EmbedData, PartnerCategory};
use crate::schema::{embed_data, partner_categories};
use crate::utils::guild_setup_check_with_reply;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;
use serenity::builder::{
	CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu,
	CreateSelectMenuKind, CreateSelectMenuOption,
};
use serenity::collector::ComponentInteractionCollector;
use serenity::model::application::{ButtonStyle, ComponentInteraction, ComponentInteractionDataKind};
use std::time::Duration;

/// Edits the partner category for an embed
#[poise::command(slash_command, guild_only)]
pub async fn edit_category(ctx: Context<'_>) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let sql_guild_id = guild.get() as i64;
	let (embeds, categories) = {
		let mut db_connection = ctx.data().db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, guild, &mut db_connection).await? {
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

	let embed_options: Vec<CreateSelectMenuOption> = embeds
		.iter()
		.map(|embed| CreateSelectMenuOption::new(&embed.embed_name, &embed.id))
		.collect();
	let category_options: Vec<CreateSelectMenuOption> = categories
		.iter()
		.map(|category| CreateSelectMenuOption::new(&category.name, &category.id))
		.collect();

	let embed_select_id = cuid2::create_id();
	let category_select_id = cuid2::create_id();
	let submit_id = cuid2::create_id();
	let cancel_id = cuid2::create_id();

	let embed_select = CreateSelectMenu::new(
		&embed_select_id,
		CreateSelectMenuKind::String { options: embed_options },
	)
	.placeholder("Embed");
	let category_select = CreateSelectMenu::new(
		&category_select_id,
		CreateSelectMenuKind::String {
			options: category_options,
		},
	)
	.placeholder("Partner Category");
	let submit_button = CreateButton::new(&submit_id)
		.label("Submit")
		.style(ButtonStyle::Primary);
	let cancel_button = CreateButton::new(&cancel_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let embed_row = CreateActionRow::SelectMenu(embed_select);
	let category_row = CreateActionRow::SelectMenu(category_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let reply = CreateReply::default().ephemeral(true).content("Select the embed to modify and the partner category list to have it use. If you want it to stop displaying a partner list, leave the partner category blank.").components(vec![embed_row, category_row, buttons_row]);

	let sent_message = ctx.send(reply).await.into_diagnostic()?;

	let mut embed = String::new();
	let mut category = String::new();

	let interaction: ComponentInteraction = loop {
		let Some(data) = ComponentInteractionCollector::new(ctx)
			.custom_ids(vec![
				embed_select_id.clone(),
				category_select_id.clone(),
				submit_id.clone(),
				cancel_id.clone(),
			])
			.timeout(Duration::from_secs(120))
			.await
		else {
			sent_message.delete(ctx).await.into_diagnostic()?;
			return Ok(());
		};

		match &data.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap_or_default();
				if data.data.custom_id == embed_select_id {
					embed = value;
				} else if data.data.custom_id == category_select_id {
					category = value;
				}
				data.create_response(ctx, CreateInteractionResponse::Acknowledge)
					.await
					.into_diagnostic()?;
			}
			ComponentInteractionDataKind::Button => {
				if data.data.custom_id == submit_id {
					break data;
				}
				sent_message.delete(ctx).await.into_diagnostic()?;
				data.create_response(ctx, CreateInteractionResponse::Acknowledge)
					.await
					.into_diagnostic()?;
				return Ok(());
			}
			_ => (),
		}
	};

	if embed.is_empty() {
		return Ok(());
	}
	let category = if category.is_empty() { None } else { Some(category) };

	let mut db_connection = ctx.data().db_connection.lock().await;
	let updated_embed: Option<EmbedData> = diesel::update(embed_data::table)
		.filter(embed_data::id.eq(embed).and(embed_data::guild.eq(sql_guild_id)))
		.set(embed_data::partner_category_list.eq(&category))
		.get_result(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let updated_embed = match updated_embed {
		Some(embed) => embed,
		None => return Ok(()), // This shouldn't happen without some client shenanigans
	};

	let display_category_data = match category {
		Some(category) => {
			let category_name: String = partner_categories::table
				.find(&category)
				.select(partner_categories::name)
				.first(&mut *db_connection)
				.into_diagnostic()?;
			format!("`{}`", category_name)
		}
		None => String::from("remove the partner category"),
	};

	let final_response = CreateInteractionResponseMessage::new()
		.content(format!(
			"The embed `{}` has been updated to {}.",
			updated_embed.embed_name, display_category_data
		))
		.components(Vec::new());
	interaction
		.create_response(ctx, CreateInteractionResponse::UpdateMessage(final_response))
		.await
		.into_diagnostic()?;

	Ok(())
}
