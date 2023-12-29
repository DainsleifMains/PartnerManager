use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::{EmbedData, PartnerCategory};
use crate::schema::{embed_data, partner_categories};
use crate::utils::guild_setup_check_with_reply;
use diesel::dsl::max;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DbError};
use miette::IntoDiagnostic;
use poise::reply::CreateReply;
use serenity::builder::{
	CreateActionRow, CreateButton, CreateInputText, CreateInteractionResponse, CreateModal, CreateSelectMenu,
	CreateSelectMenuKind, CreateSelectMenuOption,
};
use serenity::collector::{ComponentInteractionCollector, ModalInteractionCollector};
use serenity::model::application::{
	ActionRowComponent, ButtonStyle, ComponentInteraction, ComponentInteractionDataKind, InputTextStyle,
	ModalInteraction,
};
use std::time::Duration;

/// Opens a builder form to create a new embed
#[poise::command(slash_command, guild_only)]
pub async fn build_new(ctx: Context<'_>) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let sql_guild_id = guild.get() as i64;

	let partner_category_options: Vec<CreateSelectMenuOption> = {
		let mut db_connection = ctx.data().db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, guild, &mut db_connection).await? {
			return Ok(());
		}

		let categories: Vec<PartnerCategory> = partner_categories::table
			.filter(partner_categories::guild_id.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?;

		categories
			.iter()
			.map(|category| CreateSelectMenuOption::new(&category.name, &category.id))
			.collect()
	};

	let partner_category_id = cuid2::create_id();
	let submit_id = cuid2::create_id();
	let cancel_id = cuid2::create_id();
	let partner_category_input = CreateSelectMenu::new(
		&partner_category_id,
		CreateSelectMenuKind::String {
			options: partner_category_options,
		},
	);

	let submit_button = CreateButton::new(&submit_id)
		.label("Continue")
		.style(ButtonStyle::Primary);
	let cancel_button = CreateButton::new(&cancel_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);
	let buttons = vec![submit_button, cancel_button];
	let components = vec![
		CreateActionRow::SelectMenu(partner_category_input),
		CreateActionRow::Buttons(buttons),
	];
	let reply = CreateReply::default().ephemeral(true).content("# Create New Embed\n\nFirst, if this embed is for displaying a particular partner category, select that category here. Otherwise, leave the selection blank. Either way, click \"Continue\" to continue building the embed.").components(components);
	let sent_message = ctx.send(reply).await.into_diagnostic()?;

	let mut partner_category = String::new();

	let interaction: ComponentInteraction = loop {
		let Some(data) = ComponentInteractionCollector::new(ctx)
			.custom_ids(vec![partner_category_id.clone(), submit_id.clone(), cancel_id.clone()])
			.timeout(Duration::from_secs(120))
			.await
		else {
			sent_message.delete(ctx).await.into_diagnostic()?;
			return Ok(());
		};
		match &data.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				data.create_response(ctx, CreateInteractionResponse::Acknowledge)
					.await
					.into_diagnostic()?;
				let value = values.first().cloned().unwrap_or_default();
				if data.data.custom_id == partner_category_id {
					partner_category = value;
				}
			}
			ComponentInteractionDataKind::Button => {
				sent_message.delete(ctx).await.into_diagnostic()?;
				if data.data.custom_id == submit_id {
					break data;
				}
				if data.data.custom_id == cancel_id {
					data.create_response(ctx, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => (),
		}
	};

	let name_input = CreateInputText::new(InputTextStyle::Short, "Embed Name", "name")
		.placeholder("Internal name for the embed; used to reference later.")
		.max_length(100)
		.required(true);
	let embed_text_input = CreateInputText::new(InputTextStyle::Paragraph, "Text", "embed_text")
		.max_length(4000)
		.required(false);
	let image_url_input = CreateInputText::new(InputTextStyle::Short, "Image URL", "image_url").required(false);
	let color_input = CreateInputText::new(InputTextStyle::Short, "Color", "color")
		.min_length(6)
		.max_length(6)
		.required(false);

	let components = vec![
		CreateActionRow::InputText(name_input),
		CreateActionRow::InputText(embed_text_input),
		CreateActionRow::InputText(image_url_input),
		CreateActionRow::InputText(color_input),
	];
	let modal_id = cuid2::create_id();
	let modal = CreateModal::new(&modal_id, "Create New Embed").components(components);

	interaction
		.create_response(ctx, CreateInteractionResponse::Modal(modal))
		.await
		.into_diagnostic()?;

	let response = ModalInteractionCollector::new(ctx)
		.custom_ids(vec![modal_id])
		.timeout(Duration::from_secs(1200))
		.await;
	let response: ModalInteraction = match response {
		Some(data) => data,
		None => {
			sent_message.delete(ctx).await.into_diagnostic()?;
			return Ok(());
		}
	};

	let mut name = String::new();
	let mut embed_text = String::new();
	let mut image_url = String::new();
	let mut color = String::new();

	for action_row in response.data.components.iter() {
		for component in action_row.components.iter() {
			match component {
				ActionRowComponent::SelectMenu(selection_data) => {
					println!("{:?}", selection_data);
				}
				ActionRowComponent::InputText(input_data) => match input_data.custom_id.as_str() {
					"name" => name = input_data.value.clone().unwrap_or_default(),
					"embed_text" => embed_text = input_data.value.clone().unwrap_or_default(),
					"image_url" => image_url = input_data.value.clone().unwrap_or_default(),
					"color" => color = input_data.value.clone().unwrap_or_default(),
					_ => (),
				},
				_ => (),
			}
		}
	}

	response
		.create_response(ctx, CreateInteractionResponse::Acknowledge)
		.await
		.into_diagnostic()?;

	let color: Option<i32> = if color.is_empty() {
		None
	} else {
		match i32::from_str_radix(&color, 16) {
			Ok(color) => Some(color),
			Err(_) => {
				let reply = CreateReply::default()
					.ephemeral(true)
					.content("Invalid color. Colors must be valid 6-digit hexadecimal codes.");
				ctx.send(reply).await.into_diagnostic()?;
				return Ok(());
			}
		}
	};

	let mut db_connection = ctx.data().db_connection.lock().await;
	let last_embed_number: Option<i32> = embed_data::table
		.filter(embed_data::guild.eq(sql_guild_id))
		.select(max(embed_data::embed_part_sequence_number))
		.first(&mut *db_connection)
		.into_diagnostic()?;
	let next_embed_number = last_embed_number.unwrap_or(0) + 1;
	let embed_data = EmbedData {
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

	if let Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, violation_info)) = &insert_result {
		if violation_info.constraint_name() == Some("unique_embed_name_per_guild") {
			let reply = CreateReply::default()
				.ephemeral(true)
				.content(format!("The embed name `{}` is already used for another embed.", name));
			ctx.send(reply).await.into_diagnostic()?;
			return Ok(());
		}
	}
	insert_result.into_diagnostic()?;

	let reply = CreateReply::default().content(format!("Successfully added new embed: {}", name));
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
