use crate::database::get_database_connection;
use crate::models::Partner;
use crate::schema::{partner_self_users, partners};
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
use serenity::model::id::UserId;
use std::time::Duration;

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Partners command was used outside of a guild");
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

	let partner_id = cuid2::create_id();

	let partner_select = CreateSelectMenu::new(
		&partner_id,
		CreateSelectMenuKind::String {
			options: partner_select_options,
		},
	)
	.placeholder("Partner");
	let partner_row = CreateActionRow::SelectMenu(partner_select);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Select the partner for which to show our representatives.")
		.components(vec![partner_row]);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut selected_partner = String::new();

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![partner_id.clone()])
			.timeout(Duration::from_secs(30))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("No partner was selected.")
				.components(Vec::new());
			command.edit_response(&ctx.http, message).await.into_diagnostic()?;
			return Ok(());
		};
		match &interaction.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap();
				if interaction.data.custom_id == partner_id {
					if value == "<" {
						current_partner_page = current_partner_page.saturating_sub(1);
					} else if value == ">" {
						current_partner_page = (current_partner_page + 1).min(max_partner_page(&partners));
					} else {
						selected_partner = value;
						break interaction;
					}

					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;

					let partner_select_options =
						get_partners_for_page(&partners, current_partner_page, &selected_partner);
					let partner_select = CreateSelectMenu::new(
						&partner_id,
						CreateSelectMenuKind::String {
							options: partner_select_options,
						},
					)
					.placeholder("Partner");
					let partner_row = CreateActionRow::SelectMenu(partner_select);

					let message = EditInteractionResponse::new().components(vec![partner_row]);
					command.edit_response(&ctx.http, message).await.into_diagnostic()?;
				}
			}
			_ => bail!(
				"Unexpected interaction kind received for partners list_self_reps command: {:?}",
				interaction.data.kind
			),
		}
	};

	let Some(partner) = partners
		.iter()
		.find(|partner| partner.partnership_id == selected_partner)
	else {
		bail!("Partner selection list became desynchronized with the partner list");
	};
	let mut db_connection = db_connection.lock().await;

	let users: Vec<i64> = partner_self_users::table
		.filter(partner_self_users::partnership.eq(&partner.partnership_id))
		.select(partner_self_users::user_id)
		.load(&mut *db_connection)
		.into_diagnostic()?;
	let users: Vec<UserId> = users.into_iter().map(|user| UserId::new(user as u64)).collect();

	let mut message_lines: Vec<String> = Vec::with_capacity(users.len() + 1);
	message_lines.push(format!("Our representatives to {}:", partner.display_name));
	for user in users {
		message_lines.push(format!("- <@{}>", user.get()));
	}
	let message = message_lines.join("\n");
	let message = CreateInteractionResponseMessage::new()
		.content(message)
		.allowed_mentions(CreateAllowedMentions::new());
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
