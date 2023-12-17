use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::schema::partners;
use diesel::prelude::*;
use futures::Stream;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;

async fn autocomplete_partner_display_name(ctx: Context<'_>, partial: &str) -> impl Stream<Item = String> {
	let Some(guild) = ctx.guild_id() else {
		return futures::stream::iter(Vec::new());
	};

	let sql_guild_id = guild.get() as i64;
	let mut db_connection = ctx.data().db_connection.lock().await;

	let search_results: QueryResult<Vec<String>> = partners::table
		.filter(
			partners::guild
				.eq(sql_guild_id)
				.and(partners::display_name.like(format!("{}%", partial))),
		)
		.select(partners::display_name)
		.load(&mut *db_connection);
	futures::stream::iter(search_results.unwrap_or_default())
}

/// Removes a partner from the partner list
#[poise::command(slash_command, guild_only)]
pub async fn remove(
	ctx: Context<'_>,
	#[description = "The display name of the partner to remove"]
	#[autocomplete = "autocomplete_partner_display_name"]
	partner_display_name: String,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let sql_guild_id = guild.get() as i64;
	let mut db_connection = ctx.data().db_connection.lock().await;

	let delete_result = diesel::delete(partners::table)
		.filter(
			partners::guild
				.eq(sql_guild_id)
				.and(partners::display_name.eq(&partner_display_name)),
		)
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	let mut reply = CreateReply::default();
	if delete_result == 0 {
		reply = reply.ephemeral(true);
		reply = reply.content("No partner with that display name exists.");
	} else {
		reply = reply.content(format!("Removed {} as a partner.", partner_display_name));
	}
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}