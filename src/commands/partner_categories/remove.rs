use crate::database::get_database_connection;
use crate::models::PartnerCategory;
use crate::schema::{embed_data, partner_categories, partners};
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
		bail!("Partner categories remove command was run outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let partner_categories: Vec<PartnerCategory> = {
		let mut db_connection = db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
			return Ok(());
		}

		partner_categories::table
			.filter(
				partner_categories::guild_id
					.eq(sql_guild_id)
					.and(
						partners::table
							.filter(partners::category.eq(partner_categories::id))
							.count()
							.single_value()
							.eq(0),
					)
					.and(
						embed_data::table
							.filter(embed_data::partner_category_list.eq(partner_categories::id.nullable()))
							.count()
							.single_value()
							.eq(0),
					),
			)
			.load(&mut *db_connection)
			.into_diagnostic()?
	};

	if partner_categories.is_empty() {
		let message = CreateInteractionResponseMessage::new().ephemeral(true).content("No categories can be removed.\nTo be removable, a category must not have any partners in it nor be used in any embeds.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let category_select_options: Vec<CreateSelectMenuOption> = partner_categories
		.iter()
		.map(|category| CreateSelectMenuOption::new(&category.name, &category.id))
		.collect();

	let category_select_id = cuid2::create_id();
	let submit_button_id = cuid2::create_id();
	let cancel_button_id = cuid2::create_id();

	let category_select = CreateSelectMenu::new(
		&category_select_id,
		CreateSelectMenuKind::String {
			options: category_select_options,
		},
	)
	.placeholder("Partner category");
	let submit_button = CreateButton::new(&submit_button_id)
		.label("Remove")
		.style(ButtonStyle::Danger);
	let cancel_button = CreateButton::new(&cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let category_row = CreateActionRow::SelectMenu(category_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Choose the category to remove:")
		.components(vec![category_row, buttons_row]);
	command
		.create_response(&ctx, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut category_id = String::new();

	let interaction: ComponentInteraction = loop {
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
				.content("No category was removed.")
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
						.content("No category was removed.");
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Message(message))
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected interaction type received for partner_categories remove command: {:?}",
				interaction.data.kind
			),
		}
	};

	if category_id.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No category was removed; none was selected.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let Some(category) = partner_categories.iter().find(|category| category.id == category_id) else {
		bail!("Partner category selection desynchronized with partner category list");
	};

	let mut db_connection = db_connection.lock().await;
	diesel::delete(partner_categories::table)
		.filter(partner_categories::id.eq(&category_id))
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	let message =
		CreateInteractionResponseMessage::new().content(format!("Deleted the partner category {}.", category.name));
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
