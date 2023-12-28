use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::GuildSettings;
use crate::schema::{guild_settings, published_messages};
use crate::utils::GUILD_NOT_SET_UP;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;
use serenity::http::{ErrorResponse, HttpError, StatusCode};
use serenity::model::channel::{ChannelType, GuildChannel};
use serenity::model::id::ChannelId;
use serenity::prelude::SerenityError;

/// The channel to which the embed is published
#[poise::command(slash_command, guild_only, subcommands("get", "set"))]
pub async fn embed_channel(_ctx: Context<'_>) -> Result<(), CommandError> {
	Err(CommandErrorValue::BadParentCommand)?
}

/// Get the channel to which the embed is published
#[poise::command(slash_command, guild_only)]
async fn get(ctx: Context<'_>) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let mut db_connection = ctx.data().db_connection.lock().await;

	let sql_guild_id = guild.get() as i64;

	let embed_channel_id: Option<i64> = guild_settings::table
		.find(sql_guild_id)
		.select(guild_settings::publish_channel)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let embed_channel_id = match embed_channel_id {
		Some(id) => id,
		None => {
			ctx.send(CreateReply::default().content(GUILD_NOT_SET_UP).ephemeral(true))
				.await
				.into_diagnostic()?;
			return Ok(());
		}
	};

	ctx.send(
		CreateReply::default()
			.content(format!(
				"The partnership embed is published to <#{}>.",
				embed_channel_id
			))
			.ephemeral(true),
	)
	.await
	.into_diagnostic()?;

	Ok(())
}

/// Change the channel to which the embed is published
#[poise::command(slash_command, guild_only)]
async fn set(
	ctx: Context<'_>,
	#[description = "The channel in which to show the partnership embed"] embed_channel: GuildChannel,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};
	if guild != embed_channel.guild_id {
		Err(CommandErrorValue::WrongGuild)?
	}

	let mut db_connection = ctx.data().db_connection.lock().await;

	let guild_id = guild.get();
	let sql_guild_id = guild_id as i64;

	if let Err(error_message) = validate_embed_channel(&embed_channel) {
		ctx.send(CreateReply::default().content(error_message).ephemeral(true))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let guild_settings: Option<GuildSettings> = guild_settings::table
		.find(sql_guild_id)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let guild_settings = match guild_settings {
		Some(settings) => settings,
		None => {
			ctx.send(CreateReply::default().content(GUILD_NOT_SET_UP).ephemeral(true))
				.await
				.into_diagnostic()?;
			return Ok(());
		}
	};
	let current_channel_id = guild_settings.publish_channel;
	let current_messages: Vec<i64> = published_messages::table
		.filter(published_messages::guild_id.eq(sql_guild_id))
		.select(published_messages::message_id)
		.load(&mut *db_connection)
		.into_diagnostic()?;
	let current_channel_id = current_channel_id as u64;
	let current_messages: Vec<u64> = current_messages.into_iter().map(|id| id as u64).collect();
	let current_channel = ChannelId::new(current_channel_id);

	let mut message_delete_errors: Vec<SerenityError> = Vec::new();
	for message_id in current_messages {
		if let Err(error) = current_channel.delete_message(ctx, message_id).await {
			// If the message was already deleted, that's not an error
			if let SerenityError::Http(HttpError::UnsuccessfulRequest(ErrorResponse {
				status_code: StatusCode::NOT_FOUND,
				..
			})) = error
			{
				continue;
			}
			message_delete_errors.push(error);
		}
	}
	diesel::delete(published_messages::table)
		.filter(published_messages::guild_id.eq(sql_guild_id))
		.execute(&mut *db_connection)
		.into_diagnostic()?;
	if !message_delete_errors.is_empty() {
		let mut message_lines = vec![String::from("Updating the publish channel failed; the bot was unable to delete the message from the old channel. You will need to delete the messages manually."), String::from("Error details:")];
		for error in message_delete_errors {
			message_lines.push(format!("- {}", error));
		}
		ctx.send(CreateReply::default().content(message_lines.join("\n")))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let sql_channel_id = embed_channel.id.get() as i64;
	diesel::update(guild_settings::table)
		.filter(guild_settings::guild_id.eq(sql_guild_id))
		.set(guild_settings::publish_channel.eq(sql_channel_id))
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	// TODO: If we should_publish() (once that's written), publish the embed to that channel

	ctx.send(CreateReply::default().content(format!(
		"Updated embed channel from <#{}> to <#{}>!",
		current_channel_id, embed_channel.id
	)))
	.await
	.into_diagnostic()?;

	Ok(())
}

/// Validates the provided channel for use as a channel in which to post the partnership embed. Any error returned is a
/// message suitable for responding to the user who issued the command that sets the channel.
pub fn validate_embed_channel(channel: &GuildChannel) -> Result<(), String> {
	if channel.kind != ChannelType::Text {
		return Err(String::from("The embed channel must be a text channel."));
	}
	Ok(())
}
