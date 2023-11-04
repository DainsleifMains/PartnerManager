use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::GuildSettings;
use crate::schema::guild_settings;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use miette::IntoDiagnostic;
use serenity::model::channel::GuildChannel;

/// Set up the bot for a particular guild
///
/// Takes all the data required to set up the bot for your Discord server.
#[poise::command(slash_command, guild_only)]
pub async fn setup(
	ctx: Context<'_>,
	#[description = "The channel in which to show the partnership embed"] embed_channel: GuildChannel,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::BadGuild)?
	};
	if guild != embed_channel.guild_id {
		Err(CommandErrorValue::WrongGuild)?
	}

	let mut db_connection = ctx.data().db_connection.lock().await;
	let settings = GuildSettings {
		guild_id: guild.0 as i64,
		publish_channel: embed_channel.id.0 as i64,
		partner_role: None,
	};
	let insert_result = diesel::insert_into(guild_settings::table)
		.values(settings)
		.execute(&mut *db_connection);
	match insert_result {
		Ok(_) => {
			ctx.send(|reply| {
				reply.content = Some(format!(
					"Initial setup complete! Once fully configured, the partnership embed will be published to <#{}>.",
					embed_channel.id.0
				));
				reply
			})
			.await
			.into_diagnostic()?;
		}
		Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			ctx.send(|reply| {
				reply.ephemeral = true;
				reply.content = Some(String::from(
					"This server has already been set up. See `/settings` to modify individual settings.",
				));
				reply
			})
			.await
			.into_diagnostic()?;
		}
		Err(error) => Err(error).into_diagnostic()?,
	}
	Ok(())
}
