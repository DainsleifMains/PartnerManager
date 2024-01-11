use crate::database::get_database_connection;
use crate::models::EmbedData;
use crate::schema::embed_data;
use crate::sync::embed::update_embed;
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use miette::{bail, IntoDiagnostic};
use serenity::builder::{
	CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu,
	CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse,
};
use serenity::client::Context;
use serenity::collector::ComponentInteractionCollector;
use serenity::model::application::{ButtonStyle, CommandInteraction, ComponentInteractionDataKind};
use std::time::Duration;

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Partner embed command run outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let embeds: Vec<EmbedData> = {
		let mut db_connection = db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
			return Ok(());
		}

		embed_data::table
			.filter(embed_data::guild.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?
	};

	if embeds.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("You have no embeds set up.");
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

	let embed_select_id = cuid2::create_id();
	let submit_button_id = cuid2::create_id();
	let cancel_button_id = cuid2::create_id();

	let embed_select = CreateSelectMenu::new(
		&embed_select_id,
		CreateSelectMenuKind::String {
			options: embed_select_options,
		},
	);
	let cancel_button = CreateButton::new(&cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let embed_row = CreateActionRow::SelectMenu(embed_select);
	let buttons_row = CreateActionRow::Buttons(vec![cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Choose the embed that should be first:")
		.components(vec![embed_row, buttons_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut reordered_embeds: Vec<EmbedData> = Vec::with_capacity(embeds.len());

	loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				embed_select_id.clone(),
				submit_button_id.clone(),
				cancel_button_id.clone(),
			])
			.timeout(Duration::from_secs(300))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("Embed reorder canceled.")
				.components(Vec::new());
			command.edit_response(&ctx.http, message).await.into_diagnostic()?;
			return Ok(());
		};
		match &interaction.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap_or_default();
				if interaction.data.custom_id == embed_select_id {
					let Some(selected_embed) = embeds.iter().find(|embed| embed.id == value).cloned() else {
						bail!("Embed selections desynchronized from the embed list")
					};
					reordered_embeds.push(selected_embed);

					let remaining_embeds: Vec<&EmbedData> = embeds
						.iter()
						.filter(|embed| {
							!reordered_embeds
								.iter()
								.any(|reordered_embed| embed.id == reordered_embed.id)
						})
						.collect();
					let mut message_content_lines: Vec<String> = vec![String::from("Updated embed order:")];
					for (embed_index, embed) in reordered_embeds.iter().enumerate() {
						message_content_lines.push(format!("{}. {}", embed_index + 1, embed.embed_name));
					}
					let message_content = message_content_lines.join("\n");
					let message = if remaining_embeds.is_empty() {
						let submit_button = CreateButton::new(&submit_button_id)
							.label("Submit")
							.style(ButtonStyle::Primary);
						let cancel_button = CreateButton::new(&cancel_button_id)
							.label("Cancel")
							.style(ButtonStyle::Secondary);
						let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);
						EditInteractionResponse::new()
							.content(message_content)
							.components(vec![buttons_row])
					} else {
						let embed_select_options: Vec<CreateSelectMenuOption> = remaining_embeds
							.iter()
							.map(|embed| CreateSelectMenuOption::new(&embed.embed_name, &embed.id))
							.collect();

						let embed_select = CreateSelectMenu::new(
							&embed_select_id,
							CreateSelectMenuKind::String {
								options: embed_select_options,
							},
						);
						let cancel_button = CreateButton::new(&cancel_button_id)
							.label("Cancel")
							.style(ButtonStyle::Secondary);

						let embed_row = CreateActionRow::SelectMenu(embed_select);
						let buttons_row = CreateActionRow::Buttons(vec![cancel_button]);

						EditInteractionResponse::new()
							.content(message_content)
							.components(vec![embed_row, buttons_row])
					};

					command.edit_response(&ctx.http, message).await.into_diagnostic()?;
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;
				}
			}
			ComponentInteractionDataKind::Button => {
				if interaction.data.custom_id == submit_button_id {
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;
					break;
				}
				if interaction.data.custom_id == cancel_button_id {
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;
					let message = EditInteractionResponse::new()
						.content("Canceled embed reordering.")
						.components(Vec::new());
					command.edit_response(&ctx.http, message).await.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected component interaction received for partner_embed reorder command: {:?}",
				interaction.data.kind
			),
		}
	}

	{
		let mut db_connection = db_connection.lock().await;
		let embed_update: QueryResult<()> = db_connection.transaction(|db_connection| {
			for (embed_index, embed) in reordered_embeds.iter().enumerate() {
				let embed_number = (embed_index + 1) as i32;
				diesel::update(embed_data::table)
					.filter(embed_data::id.eq(&embed.id))
					.set(embed_data::embed_part_sequence_number.eq(embed_number))
					.execute(db_connection)?;
			}

			Ok(())
		});
		embed_update.into_diagnostic()?;

		let message = EditInteractionResponse::new().components(Vec::new());
		command.edit_response(&ctx.http, message).await.into_diagnostic()?;
	}

	update_embed(ctx, guild).await?;

	Ok(())
}
