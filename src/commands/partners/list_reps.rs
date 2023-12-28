use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::schema::{partner_users, partners};
use crate::utils::autocomplete::partner_display_name;
use crate::utils::guild_setup_check_with_reply;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;

/// Lists representatives for a particular partner server
#[poise::command(slash_command, guild_only)]
pub async fn list_reps(
	ctx: Context<'_>,
	#[description = "Name of the partner server"]
	#[autocomplete = "partner_display_name"]
	partner_server_name: String,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let mut db_connection = ctx.data().db_connection.lock().await;
	if !guild_setup_check_with_reply(ctx, guild, &mut db_connection).await? {
		return Ok(());
	}
	let sql_guild_id = guild.get() as i64;

	let partnership_id: Option<String> = partners::table
		.filter(
			partners::guild
				.eq(sql_guild_id)
				.and(partners::display_name.eq(&partner_server_name)),
		)
		.select(partners::partnership_id)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let Some(partnership_id) = partnership_id else {
		let mut reply = CreateReply::default();
		reply = reply.ephemeral(true);
		reply = reply.content(format!("You have no partner server named `{}`.", partner_server_name));
		ctx.send(reply).await.into_diagnostic()?;
		return Ok(());
	};

	let rep_user_ids: Vec<i64> = partner_users::table
		.filter(partner_users::partnership_id.eq(partnership_id))
		.select(partner_users::user_id)
		.load(&mut *db_connection)
		.into_diagnostic()?;

	let reply_content = if rep_user_ids.is_empty() {
		format!("There are no representatives for `{}`.", partner_server_name)
	} else {
		let mut message_parts = vec![format!("Partner representatives for `{}`:", partner_server_name)];
		for user_id in rep_user_ids.iter() {
			let user_id = *user_id as u64;
			message_parts.push(format!("\n- <@{}>", user_id));
		}
		message_parts.join("")
	};

	ctx.send(CreateReply::default().content(reply_content))
		.await
		.into_diagnostic()?;

	Ok(())
}
