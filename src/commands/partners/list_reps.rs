use crate::database::get_database_connection;
use crate::models::Partner;
use crate::schema::{partner_users, partners};
use crate::utils::pagination::{get_partners_for_page, max_partner_page};
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use miette::{bail, IntoDiagnostic};
use serenity::builder::{
	CreateActionRow, CreateAllowedMentions, CreateInteractionResponse, CreateInteractionResponseMessage,
	CreateSelectMenu, CreateSelectMenuKind, EditInteractionResponse,
};
use serenity::client::Context;
use serenity::collector::ComponentInteractionCollector;
use serenity::model::application::{CommandInteraction, ComponentInteraction, ComponentInteractionDataKind};
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
			.order(partners::display_name.asc())
			.load(&mut *db_connection)
			.into_diagnostic()?
	};

	if partners.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("You have no partners for which to list representatives.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let mut current_partner_page = 0;
	let partner_select_options = get_partners_for_page(&partners, current_partner_page, "");

	let partner_select_id = cuid2::create_id();
	let partner_select = CreateSelectMenu::new(
		&partner_select_id,
		CreateSelectMenuKind::String {
			options: partner_select_options,
		},
	);
	let partner_row = CreateActionRow::SelectMenu(partner_select);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Choose the partner for which to list representatives.")
		.components(vec![partner_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let (interaction, partner_id): (ComponentInteraction, String) = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![partner_select_id.clone()])
			.timeout(Duration::from_secs(30))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("Selection timed out.")
				.components(Vec::new());
			command.edit_response(&ctx.http, message).await.into_diagnostic()?;
			return Ok(());
		};
		match &interaction.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap_or_default();
				if interaction.data.custom_id == partner_select_id {
					if value == "<" {
						current_partner_page = current_partner_page.saturating_sub(1);
					} else if value == ">" {
						current_partner_page = (current_partner_page + 1).min(max_partner_page(&partners));
					} else {
						break (interaction, value);
					}

					let partner_select_options = get_partners_for_page(&partners, current_partner_page, "");
					let partner_select = CreateSelectMenu::new(
						&partner_select_id,
						CreateSelectMenuKind::String {
							options: partner_select_options,
						},
					);
					let partner_row = CreateActionRow::SelectMenu(partner_select);

					let message = EditInteractionResponse::new().components(vec![partner_row]);
					command.edit_response(&ctx.http, message).await.into_diagnostic()?;
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;
				}
			}
			_ => bail!(
				"Unexpected interaction occurred in partner list_reps command: {:?}",
				interaction.data.kind
			),
		}
	};

	let mut db_connection = db_connection.lock().await;
	let partner_data: Option<Partner> = partners::table
		.filter(
			partners::partnership_id
				.eq(&partner_id)
				.and(partners::guild.eq(sql_guild_id)),
		)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let Some(partner_data) = partner_data else {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("The selected partner is not valid.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	};

	let rep_user_ids: Vec<i64> = partner_users::table
		.filter(partner_users::partnership_id.eq(&partner_id))
		.select(partner_users::user_id)
		.load(&mut *db_connection)
		.into_diagnostic()?;

	let message_content = if rep_user_ids.is_empty() {
		format!("There are no representatives for {}.", partner_data.display_name)
	} else {
		let mut message_lines = vec![format!("Partner representatives for {}:", partner_data.display_name)];
		for user_id in rep_user_ids {
			let user_id = user_id as u64;
			message_lines.push(format!("- <@{}>", user_id));
		}
		message_lines.join("\n")
	};

	let message = CreateInteractionResponseMessage::new()
		.content(message_content)
		.allowed_mentions(CreateAllowedMentions::new());
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
