use crate::database::get_database_connection;
use crate::models::{EmbedData, PartnerCategory};
use crate::schema::{embed_data, partner_categories};
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::dsl::max;
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
	let Some(guild) = command.guild_id else {
		bail!("Partner embed command was used outside of a guild");
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

	let (partner_category, category_interaction) = if partner_categories.is_empty() {
		(String::new(), None)
	} else {
		let partner_category_options: Vec<CreateSelectMenuOption> = partner_categories
			.iter()
			.map(|category| CreateSelectMenuOption::new(&category.name, &category.id))
			.collect();

		let category_select_id = cuid2::create_id();
		let submit_button_id = cuid2::create_id();
		let cancel_button_id = cuid2::create_id();

		let category_select = CreateSelectMenu::new(
			&category_select_id,
			CreateSelectMenuKind::String {
				options: partner_category_options,
			},
		);
		let submit_button = CreateButton::new(&submit_button_id)
			.label("Continue")
			.style(ButtonStyle::Primary);
		let cancel_button = CreateButton::new(&cancel_button_id)
			.label("Cancel")
			.style(ButtonStyle::Secondary);

		let category_row = CreateActionRow::SelectMenu(category_select);
		let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

		let message = CreateInteractionResponseMessage::new().ephemeral(true).content("# Create New Embed\n\nFirst, if this embed is for displaying a particular partner category, select that category here. Otherwise, leave the selection blank. Either way, click \"Continue\" to continue building the embed.").components(vec![category_row, buttons_row]);
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;

		let mut category_id = String::new();

		let category_interaction: ComponentInteraction = loop {
			let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
				.custom_ids(vec![
					category_select_id.clone(),
					submit_button_id.clone(),
					cancel_button_id.clone(),
				])
				.timeout(Duration::from_secs(30))
				.await
			else {
				let message = EditInteractionResponse::new()
					.content("Embed was not created.")
					.components(Vec::new());
				command.edit_response(&ctx.http, message).await.into_diagnostic()?;
				return Ok(());
			};
			match &interaction.data.kind {
				ComponentInteractionDataKind::StringSelect { values } => {
					let value = values.first().cloned().unwrap_or_default();
					if interaction.data.custom_id == category_select_id {
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
							.content("Embed was not created.");
						interaction
							.create_response(&ctx.http, CreateInteractionResponse::Message(message))
							.await
							.into_diagnostic()?;
						return Ok(());
					}
				}
				_ => bail!(
					"Unexpected interaction data type received by partner_embed build_new command: {:?}",
					interaction.data.kind
				),
			}
		};

		if !category_id.is_empty() && !partner_categories.iter().any(|category| category.id == category_id) {
			bail!("Partner category selection desynchronized with the partner category list");
		}

		(category_id, Some(category_interaction))
	};

	let name_input = CreateInputText::new(InputTextStyle::Short, "Embed Name", "")
		.placeholder("Internal name for the embed; used for reference later")
		.max_length(100)
		.required(true);
	let embed_text_input = CreateInputText::new(InputTextStyle::Paragraph, "Text", "")
		.max_length(4000)
		.required(false);
	let image_url_input = CreateInputText::new(InputTextStyle::Short, "Image URL", "").required(false);
	let color_input = CreateInputText::new(InputTextStyle::Short, "Color", "")
		.min_length(6)
		.max_length(6)
		.required(false);

	let modal = CreateQuickModal::new("Create New Embed")
		.timeout(Duration::from_secs(900))
		.field(name_input)
		.field(embed_text_input)
		.field(image_url_input)
		.field(color_input);
	let modal_response = if let Some(interaction) = category_interaction {
		interaction.quick_modal(ctx, modal).await
	} else {
		command.quick_modal(ctx, modal).await
	}
	.into_diagnostic()?;

	let Some(modal_response) = modal_response else {
		return Ok(());
	};
	let (modal_interaction, modal_inputs) = (modal_response.interaction, modal_response.inputs);
	let mut inputs_iter = modal_inputs.into_iter();
	let Some(name) = inputs_iter.next() else {
		bail!("Required embed name was not entered");
	};
	let embed_text = inputs_iter.next().unwrap_or_default();
	let image_url = inputs_iter.next().unwrap_or_default();
	let color = inputs_iter.next().unwrap_or_default();

	let color: Option<i32> = if color.is_empty() {
		None
	} else {
		match i32::from_str_radix(&color, 16) {
			Ok(color) => Some(color),
			Err(_) => {
				let message = CreateInteractionResponseMessage::new()
					.ephemeral(true)
					.content("The entered color is invalid.");
				modal_interaction
					.create_response(&ctx.http, CreateInteractionResponse::Message(message))
					.await
					.into_diagnostic()?;
				return Ok(());
			}
		}
	};

	let mut db_connection = db_connection.lock().await;
	let last_embed_number: Option<i32> = embed_data::table
		.filter(embed_data::guild.eq(sql_guild_id))
		.select(max(embed_data::embed_part_sequence_number))
		.first(&mut *db_connection)
		.into_diagnostic()?;
	let next_embed_number = last_embed_number.unwrap_or(0) + 1;
	let embed_data = EmbedData {
		id: cuid2::create_id(),
		guild: sql_guild_id,
		embed_part_sequence_number: next_embed_number,
		embed_name: name.clone(),
		partner_category_list: if partner_category.is_empty() {
			None
		} else {
			Some(partner_category)
		},
		embed_text,
		image_url,
		color,
	};

	let insert_result = diesel::insert_into(embed_data::table)
		.values(embed_data)
		.execute(&mut *db_connection);
	let message = match insert_result {
		Ok(_) => CreateInteractionResponseMessage::new().content(format!("Successfully added new embed: {}", name)),
		Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, violation_info)) => {
			if violation_info.constraint_name() == Some("unique_embed_name_per_guild") {
				CreateInteractionResponseMessage::new()
					.ephemeral(true)
					.content(format!("The embed name {} is already in use for another embed.", name))
			} else {
				bail!(DbError::DatabaseError(
					DatabaseErrorKind::UniqueViolation,
					violation_info
				));
			}
		}
		Err(error) => bail!(error),
	};
	modal_interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
