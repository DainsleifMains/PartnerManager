use crate::schema::guild_settings;
use diesel::dsl::count_star;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::client::Context;
use serenity::model::application::CommandInteraction;
use serenity::model::id::GuildId;

pub const GUILD_NOT_SET_UP: &str = "This server hasn't been set up yet; use `/setup` to set up this server.";

/// Checks whether a guild has set up the bot. Only necessary if the guild_settings table isn't queried anyway.
fn guild_is_set_up(guild: GuildId, db_connection: &mut PgConnection) -> miette::Result<bool> {
	let sql_guild_id = guild.get() as i64;
	let guild_count: i64 = guild_settings::table
		.filter(guild_settings::guild_id.eq(sql_guild_id))
		.select(count_star())
		.first(db_connection)
		.into_diagnostic()?;
	Ok(guild_count > 0)
}

/// Checks whether a guild has set up the bot. Automatically replies with the standard response if the server isn't set
/// up. Only necessary if the guild_settings table isn't queried anyway.
pub async fn guild_setup_check_with_reply(
	ctx: &Context,
	command: &CommandInteraction,
	guild: GuildId,
	db_connection: &mut PgConnection,
) -> miette::Result<bool> {
	let set_up = guild_is_set_up(guild, db_connection)?;
	if !set_up {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content(GUILD_NOT_SET_UP);
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
	}
	Ok(set_up)
}
