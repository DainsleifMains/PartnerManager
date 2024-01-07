use crate::database::get_database_connection;
use crate::models::EmbedData;
use crate::schema::embed_data;
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
	)
	.placeholder("Embed");
	let submit_button = CreateButton::new(&submit_button_id)
		.label("Remove")
		.style(ButtonStyle::Danger);
	let cancel_button = CreateButton::new(&cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let embed_row = CreateActionRow::SelectMenu(embed_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Select the embed to remove:")
		.components(vec![embed_row, buttons_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut embed_id = String::new();

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				embed_select_id.clone(),
				submit_button_id.clone(),
				cancel_button_id.clone(),
			])
			.timeout(Duration::from_secs(60))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("No embed was removed.")
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
				}
			}
			ComponentInteractionDataKind::Button => {
				if interaction.data.custom_id == submit_button_id {
					break interaction;
				}
				if interaction.data.custom_id == cancel_button_id {
					let message = CreateInteractionResponseMessage::new()
						.ephemeral(true)
						.content("Canceled removing an embed.");
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Message(message))
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected component interaction type received for partner embed remove command: {:?}",
				interaction.data.kind
			),
		}
	};

	if embed_id.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No embed was removed.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let Some(removing_embed) = embeds.iter().find(|embed| embed.id == embed_id) else {
		bail!("Embed selections desynchronized from embeds list");
	};

	let mut db_connection = db_connection.lock().await;
	let delete_result: QueryResult<()> = db_connection.transaction(|db_connection| {
		diesel::delete(embed_data::table)
			.filter(embed_data::id.eq(&embed_id))
			.execute(db_connection)?;

		let remaining_embeds: Vec<EmbedData> = embed_data::table
			.filter(embed_data::guild.eq(sql_guild_id))
			.order(embed_data::embed_part_sequence_number.asc())
			.load(db_connection)?;
		for (embed_index, embed) in remaining_embeds.iter().enumerate() {
			let embed_number = (embed_index + 1) as i32;
			diesel::update(embed_data::table)
				.filter(embed_data::id.eq(&embed.id))
				.set(embed_data::embed_part_sequence_number.eq(embed_number))
				.execute(db_connection)?;
		}

		Ok(())
	});
	delete_result.into_diagnostic()?;

	let message =
		CreateInteractionResponseMessage::new().content(format!("Removed the embed {}.", removing_embed.embed_name));
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
