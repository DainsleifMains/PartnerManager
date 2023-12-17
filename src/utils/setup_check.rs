use crate::command_types::{CommandError, Context};
use crate::schema::guild_settings;
use diesel::dsl::count_star;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;
use serenity::model::id::GuildId;

pub const GUILD_NOT_SET_UP: &str = "This server hasn't been set up yet; use `/setup` to set up this server.";

/// Checks whether a guild has set up the bot. Only necessary if the guild_settings table isn't queried anyway.
pub fn guild_is_set_up(guild: GuildId, db_connection: &mut PgConnection) -> QueryResult<bool> {
	let sql_guild_id = guild.get() as i64;
	let guild_count: i64 = guild_settings::table
		.filter(guild_settings::guild_id.eq(sql_guild_id))
		.select(count_star())
		.first(db_connection)?;
	Ok(guild_count > 0)
}

pub async fn guild_setup_check_with_reply(
	ctx: Context<'_>,
	guild: GuildId,
	db_connection: &mut PgConnection,
) -> Result<bool, CommandError> {
	let set_up = guild_is_set_up(guild, db_connection).into_diagnostic()?;
	if !set_up {
		let mut reply = CreateReply::default();
		reply = reply.ephemeral(true);
		reply = reply.content(GUILD_NOT_SET_UP);
		ctx.send(reply).await.into_diagnostic()?;
	}
	Ok(set_up)
}
