use crate::database::get_database_connection;
use crate::models::EmbedData;
use crate::schema::embed_data;
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DbError};
use miette::{bail, IntoDiagnostic};
use serenity::builder::{
	CreateActionRow, CreateButton, CreateInputText, CreateInteractionResponse, CreateInteractionResponseMessage,
	CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse,
};
use serenity::client::Context;
use serenity::collector::ComponentInteractionCollector;
use serenity::model::application::{
	ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionDataKind, InputTextStyle,
};
use serenity::utils::CreateQuickModal;
use std::time::Duration;

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild_id) = command.guild_id else {
		bail!("Partner embed command was run outside of a guild");
	};

	let sql_guild_id = guild_id.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let embeds: Vec<EmbedData> = {
		let mut db_connection = db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, command, guild_id, &mut db_connection).await? {
			return Ok(());
		}

		embed_data::table
			.filter(embed_data::guild.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?
	};

	let embed_select_id = cuid2::create_id();
	let submit_button_id = cuid2::create_id();
	let cancel_button_id = cuid2::create_id();

	let embed_select_options: Vec<CreateSelectMenuOption> = embeds
		.iter()
		.map(|embed| CreateSelectMenuOption::new(&embed.embed_name, &embed.id))
		.collect();

	let embed_select = CreateSelectMenu::new(
		&embed_select_id,
		CreateSelectMenuKind::String {
			options: embed_select_options,
		},
	)
	.placeholder("Embed");
	let submit_button = CreateButton::new(&submit_button_id)
		.label("Edit")
		.style(ButtonStyle::Primary);
	let cancel_button = CreateButton::new(&cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let embed_row = CreateActionRow::SelectMenu(embed_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Select the embed to edit:")
		.components(vec![embed_row, buttons_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut selected_embed_id = String::new();

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				embed_select_id.clone(),
				submit_button_id.clone(),
				cancel_button_id.clone(),
			])
			.timeout(Duration::from_secs(30))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("Canceled embed edit.")
				.components(Vec::new());
			command.edit_response(&ctx.http, message).await.into_diagnostic()?;
			return Ok(());
		};
		match &interaction.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap_or_default();
				if interaction.data.custom_id == embed_select_id {
					selected_embed_id = value;
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
					let message = EditInteractionResponse::new()
						.content("Canceled embed edit.")
						.components(Vec::new());
					command.edit_response(&ctx.http, message).await.into_diagnostic()?;
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected interaction type received for partner_embed edit_content command: {:?}",
				interaction.data.kind
			),
		}
	};

	let Some(embed) = embeds.iter().find(|embed| embed.id == selected_embed_id) else {
		bail!("Embed selection desynchronized with the available embed list");
	};

	let embed_name_input = CreateInputText::new(InputTextStyle::Short, "Embed Name", "")
		.required(true)
		.max_length(100)
		.value(&embed.embed_name);
	let embed_text_input = CreateInputText::new(InputTextStyle::Paragraph, "Text", "")
		.required(false)
		.max_length(4000)
		.value(&embed.embed_text);
	let image_url_input = CreateInputText::new(InputTextStyle::Short, "Image URL", "")
		.required(false)
		.value(&embed.image_url);
	let mut color_input = CreateInputText::new(InputTextStyle::Short, "Color", "")
		.required(false)
		.min_length(6)
		.max_length(6);

	if let Some(color) = embed.color {
		color_input = color_input.value(format!("{:06x}", color));
	}

	let modal = CreateQuickModal::new("Edit Embed")
		.field(embed_name_input)
		.field(embed_text_input)
		.field(image_url_input)
		.field(color_input)
		.timeout(Duration::from_secs(600));
	let modal_response = interaction.quick_modal(ctx, modal).await.into_diagnostic()?;

	let Some(modal_response) = modal_response else {
		let message = EditInteractionResponse::new()
			.content("Canceled embed edit.")
			.components(Vec::new());
		command.edit_response(&ctx.http, message).await.into_diagnostic()?;
		return Ok(());
	};

	let modal_inputs = modal_response.inputs;
	let (new_embed_name, new_embed_text, new_image_url, new_color) =
		(&modal_inputs[0], &modal_inputs[1], &modal_inputs[2], &modal_inputs[3]);

	let new_color = if new_color.is_empty() {
		None
	} else {
		match i32::from_str_radix(new_color, 16) {
			Ok(color) => Some(color),
			Err(_) => {
				let message = CreateInteractionResponseMessage::new()
					.ephemeral(true)
					.content("The entered color was invalid.");
				modal_response
					.interaction
					.create_response(&ctx.http, CreateInteractionResponse::Message(message))
					.await
					.into_diagnostic()?;
				return Ok(());
			}
		}
	};

	let mut db_connection = db_connection.lock().await;
	let update_result = diesel::update(embed_data::table)
		.filter(embed_data::id.eq(&embed.id))
		.set((
			embed_data::embed_name.eq(new_embed_name),
			embed_data::embed_text.eq(new_embed_text),
			embed_data::image_url.eq(new_image_url),
			embed_data::color.eq(new_color),
		))
		.execute(&mut *db_connection);
	match update_result {
		Ok(_) => {
			let message = CreateInteractionResponseMessage::new()
				.content(format!("Successfully updated the embed {}.", new_embed_name));
			modal_response
				.interaction
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
		}
		Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, violation_info)) => {
			let message_content = if violation_info.constraint_name() == Some("unique_embed_name_per_guild") {
				"That embed name is already in use for a different embed."
			} else {
				"An unknown collision occurred with an existing embed."
			};
			let message = CreateInteractionResponseMessage::new()
				.ephemeral(true)
				.content(message_content);
			modal_response
				.interaction
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
		}
		Err(error) => bail!(error),
	}

	Ok(())
}
