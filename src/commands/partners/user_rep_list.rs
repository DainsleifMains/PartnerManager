use crate::database::get_database_connection;
use crate::schema::{partner_self_users, partner_users, partners};
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use miette::{bail, ensure, IntoDiagnostic, Severity};
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::client::Context;
use serenity::model::application::{CommandInteraction, ResolvedOption, ResolvedValue};

pub async fn execute(
	ctx: &Context,
	command: &CommandInteraction,
	options: &[ResolvedOption<'_>],
) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Partners command executed outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;

	let Some(user_option) = options.first() else {
		bail!("Insufficient options passed to partners user_rep_list command");
	};
	ensure!(
		user_option.name == "user",
		severity = Severity::Error,
		"Wrong option passed to partners user_rep_list command: {:?}",
		user_option
	);
	let ResolvedValue::User(user, _) = user_option.value else {
		bail!("Incorrect type provided for user option: {:?}", user_option.value);
	};

	let sql_user_id = user.id.get() as i64;

	let mut db_connection = db_connection.lock().await;
	if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
		return Ok(());
	}

	let partner_names: Vec<String> = partners::table
		.filter(
			partners::guild.eq(sql_guild_id).and(
				partners::partnership_id.eq_any(
					partner_users::table
						.filter(partner_users::user_id.eq(sql_user_id))
						.select(partner_users::partnership_id),
				),
			),
		)
		.select(partners::display_name)
		.load(&mut *db_connection)
		.into_diagnostic()?;
	let partner_self_names: Vec<String> = partners::table
		.filter(
			partners::guild.eq(sql_guild_id).and(
				partners::partnership_id.eq_any(
					partner_self_users::table
						.filter(partner_self_users::user_id.eq(sql_user_id))
						.select(partner_self_users::partnership),
				),
			),
		)
		.select(partners::display_name)
		.load(&mut *db_connection)
		.into_diagnostic()?;

	let mut message_lines: Vec<String> = Vec::new();
	if !partner_names.is_empty() {
		message_lines.push(format!("<@{}> represents the following partners:", user.id.get()));
		for name in partner_names {
			message_lines.push(format!("- {}", name));
		}
	}
	if !partner_self_names.is_empty() {
		message_lines.push(format!("<@{}> represents us to the following partners:", user.id.get()));
		for name in partner_self_names {
			message_lines.push(format!("- {}", name));
		}
	}

	if message_lines.is_empty() {
		message_lines.push(format!("<@{}> does not represent any partners.", user.id.get()));
	}

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content(message_lines.join("\n"));
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
