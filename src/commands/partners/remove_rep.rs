use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::schema::{partner_users, partners};
use crate::utils::autocomplete::partner_display_name;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;
use serenity::model::id::UserId;

/// Removes a representative for a partner
#[poise::command(slash_command, guild_only)]
pub async fn remove_rep(
	ctx: Context<'_>,
	#[description = "The partner for which to remove the representative"]
	#[autocomplete = "partner_display_name"]
	partner_display_name: String,
	#[description = "The user to remove as a representative"] user: UserId,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let mut db_connection = ctx.data().db_connection.lock().await;
	let sql_guild_id = guild.get() as i64;

	let partnership_id: Option<String> = partners::table
		.filter(
			partners::guild
				.eq(sql_guild_id)
				.and(partners::display_name.eq(&partner_display_name)),
		)
		.select(partners::partnership_id)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let Some(partnership_id) = partnership_id else {
		let mut reply = CreateReply::default();
		reply = reply.ephemeral(true);
		reply = reply.content(format!("You have no partner named `{}`.", partner_display_name));
		ctx.send(reply).await.into_diagnostic()?;
		return Ok(());
	};

	let sql_user_id = user.get() as i64;
	let delete_count = diesel::delete(partner_users::table)
		.filter(
			partner_users::partnership_id
				.eq(partnership_id)
				.and(partner_users::user_id.eq(sql_user_id)),
		)
		.execute(&mut *db_connection)
		.into_diagnostic()?;
	
	// TODO role sync

	let mut reply = CreateReply::default();
	if delete_count == 0 {
		reply = reply.ephemeral(true);
		reply = reply.content(format!(
			"<@{}> wasn't a partner representative for `{}`.",
			user.get(),
			partner_display_name
		));
	} else {
		reply = reply.content(format!(
			"Removed <@{}> as a partner for `{}`.",
			user.get(),
			partner_display_name
		));
	}
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
