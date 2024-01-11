use crate::database::get_database_connection;
use crate::models::GuildSettings;
use crate::schema::{guild_settings, published_messages};
use crate::sync::embed::{remove_embed, update_embed};
use crate::utils::setup_check::GUILD_NOT_SET_UP;
use diesel::prelude::*;
use miette::{bail, ensure, IntoDiagnostic, Severity};
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::client::Context;
use serenity::http::{ErrorResponse, HttpError, StatusCode};
use serenity::model::application::{CommandInteraction, ResolvedOption, ResolvedValue};
use serenity::model::id::ChannelId;
use serenity::prelude::SerenityError;

pub async fn execute(
	ctx: &Context,
	command: &CommandInteraction,
	options: &[ResolvedOption<'_>],
) -> miette::Result<()> {
	let Some(option) = options.first() else {
		bail!("Insufficient subcommands for settings embed_channel command");
	};

	let ResolvedValue::SubCommand(subcommand_options) = &option.value else {
		bail!("Incorrect data type for settings embed_channel subcommand");
	};
	match option.name {
		"get" => get(ctx, command).await,
		"set" => set(ctx, command, subcommand_options).await,
		_ => bail!("Invalid subcommand passed to settings embed_channel command"),
	}
}

async fn get(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Settings command was used outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let mut db_connection = db_connection.lock().await;

	let embed_channel_id: Option<i64> = guild_settings::table
		.find(sql_guild_id)
		.select(guild_settings::publish_channel)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;

	let message = match embed_channel_id {
		Some(id) => CreateInteractionResponseMessage::new()
			.content(format!("The partnership embed is published to <#{}>.", id))
			.ephemeral(true),
		None => CreateInteractionResponseMessage::new()
			.content(GUILD_NOT_SET_UP)
			.ephemeral(true),
	};
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}

async fn set(ctx: &Context, command: &CommandInteraction, options: &[ResolvedOption<'_>]) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Settings command was used outside of a guild");
	};

	let Some(embed_channel_option) = options.first() else {
		bail!("Incorrect data type for options to settings embed_channel set command");
	};
	ensure!(
		embed_channel_option.name == "embed_channel",
		severity = Severity::Error,
		"wrong option passed to the settings embed_channel set command"
	);
	let ResolvedValue::Channel(embed_channel) = embed_channel_option.value else {
		bail!("Channel option got a non-channel value: {:?}", embed_channel_option);
	};
	let embed_channel = embed_channel.id.to_channel(&ctx.http).await.into_diagnostic()?;
	let Some(embed_channel) = embed_channel.guild() else {
		bail!("non-guild channel passed as the embed channel");
	};

	if embed_channel.guild_id != guild {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("The provided channel isn't in this server.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let (current_channel_id, message_delete_errors) = {
		let mut db_connection = db_connection.lock().await;

		let guild_settings: Option<GuildSettings> = guild_settings::table
			.find(sql_guild_id)
			.first(&mut *db_connection)
			.optional()
			.into_diagnostic()?;
		let Some(guild_settings) = guild_settings else {
			let message = CreateInteractionResponseMessage::new()
				.ephemeral(true)
				.content(GUILD_NOT_SET_UP);
			command
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
			return Ok(());
		};
		let current_channel_id = guild_settings.publish_channel;
		let current_messages: Vec<i64> = published_messages::table
			.filter(published_messages::guild_id.eq(sql_guild_id))
			.select(published_messages::message_id)
			.load(&mut *db_connection)
			.into_diagnostic()?;
		let current_channel_id = current_channel_id as u64;
		let current_messages: Vec<u64> = current_messages
			.into_iter()
			.map(|message_id| message_id as u64)
			.collect();
		let current_channel = ChannelId::new(current_channel_id);

		let mut message_delete_errors: Vec<SerenityError> = Vec::new();
		for message_id in current_messages {
			if let Err(error) = current_channel.delete_message(&ctx.http, message_id).await {
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

		(current_channel_id, message_delete_errors)
	};

	if !message_delete_errors.is_empty() {
		let mut message_lines = vec![String::from("Updating the publish channel failed; the bot was unable to delete the message from the old channel. You will need to delete the messages manually.")];
		for error in message_delete_errors {
			message_lines.push(format!("- {}", error));
		}
		let message = CreateInteractionResponseMessage::new().content(message_lines.join("\n"));
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	remove_embed(ctx, guild).await?;

	{
		let mut db_connection = db_connection.lock().await;
		let sql_channel_id = embed_channel.id.get() as i64;
		diesel::update(guild_settings::table)
			.filter(guild_settings::guild_id.eq(sql_guild_id))
			.set(guild_settings::publish_channel.eq(sql_channel_id))
			.execute(&mut *db_connection)
			.into_diagnostic()?;
	}

	update_embed(ctx, guild).await?;

	let message = CreateInteractionResponseMessage::new().content(format!(
		"Updated embed channel from <#{}>, to <#{}>.",
		current_channel_id,
		embed_channel.id.get()
	));
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
