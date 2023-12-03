use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::GuildSettings;
use crate::schema::guild_settings;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use serenity::model::channel::GuildChannel;
use serenity::model::id::ChannelId;

#[poise::command(prefix_command, slash_command)]
pub async fn embed_channel(
	ctx: Context<'_>,
	#[description = "The channel in which to show the partnership embed"] embed_channel: Option<GuildChannel>,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::BadGuild)?
	};
	if let Some(channel) = embed_channel.as_ref() {
		if guild != channel.guild_id {
			Err(CommandErrorValue::WrongGuild)?
		}
	}

	let mut db_connection = ctx.data().db_connection.lock().await;

	let guild_id = guild.0;
	let sql_guild_id = guild_id as i64;

	let Some(embed_channel) = embed_channel else {
		let role: Option<i64> = guild_settings::table
			.find(sql_guild_id)
			.select(guild_settings::partner_role)
			.first(&mut *db_connection)
			.into_diagnostic()?;
		let role = role.map(|id| id as u64);
		ctx.send(|reply| {
			reply.ephemeral = true;
			reply.content = Some(if let Some(role_id) = role {
				format!("The current partner role is <@{}>.", role_id)
			} else {
				String::from("There is no partner role.")
			});
			reply
		})
		.await
		.into_diagnostic()?;
		return Ok(());
	};

	let guild_settings: Option<GuildSettings> = guild_settings::table
		.find(sql_guild_id)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let guild_settings = match guild_settings {
		Some(settings) => settings,
		None => {
			ctx.send(|reply| {
				reply.content = Some(String::from(
					"This server hasn't been set up yet; use `/setup` to set up this server.",
				));
				reply.ephemeral = true;
				reply
			})
			.await
			.into_diagnostic()?;
			return Ok(());
		}
	};
	let current_channel_id = guild_settings.publish_channel;
	let current_message_id = guild_settings.published_message_id;
	let current_channel_id = current_channel_id as u64;
	let current_message_id = current_message_id.map(|id| id as u64);
	let current_channel = ChannelId(current_channel_id);

	if let Some(message_id) = current_message_id {
		// We clear the message ID from the database first so that if manual deletion is required, the user is able to
		// handle that without us breaking next time due to the message not existing
		let no_message: Option<i64> = None;
		diesel::update(guild_settings::table)
			.filter(guild_settings::guild_id.eq(sql_guild_id))
			.set(guild_settings::published_message_id.eq(no_message))
			.execute(&mut *db_connection)
			.into_diagnostic()?;
		let message_delete_result = current_channel.delete_message(ctx, message_id).await;
		if let Err(error) = message_delete_result {
			ctx.send(|reply| {
				reply.ephemeral = true;
				reply.content = Some(format!("Updating the publish channel failed; the bot was unable to delete the message from the old channel. You will need to delete that message manually. (Error details: {})", error));
				reply
			}).await.into_diagnostic()?;
			return Ok(());
		}
	}

	let sql_channel_id = embed_channel.id.0 as i64;
	diesel::update(guild_settings::table)
		.filter(guild_settings::guild_id.eq(sql_guild_id))
		.set(guild_settings::publish_channel.eq(sql_channel_id))
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	// TODO: If we should_publish() (once that's written), publish the embed to that channel

	ctx.send(|reply| {
		reply.ephemeral = true;
		reply.content = Some(format!(
			"Updated embed channel from <#{}> to <#{}>!",
			current_channel_id, embed_channel.id
		));
		reply
	})
	.await
	.into_diagnostic()?;

	Ok(())
}
