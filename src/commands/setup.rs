use crate::database::get_database_connection;
use crate::models::GuildSettings;
use crate::schema::guild_settings;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use miette::{bail, ensure, IntoDiagnostic, Severity};
use serenity::builder::{
	CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::client::Context;
use serenity::model::application::{CommandInteraction, CommandOptionType, CommandType, ResolvedValue};
use serenity::model::channel::ChannelType;
use serenity::model::permissions::Permissions;

pub fn definition() -> CreateCommand {
	CreateCommand::new("setup")
		.kind(CommandType::ChatInput)
		.default_member_permissions(Permissions::MANAGE_GUILD)
		.dm_permission(false)
		.description("Set up the bot for a particular guild")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::Channel,
				"embed_channel",
				"The channel in which to show the partnership embed",
			)
			.required(true)
			.channel_types(vec![ChannelType::Text]),
		)
}

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Setup command was used outside of a guild");
	};

	let options = command.data.options();
	ensure!(
		options.len() == 1,
		severity = Severity::Error,
		"incorrect number of options received"
	);
	let channel_option = options.first().unwrap();
	ensure!(
		channel_option.name == "embed_channel",
		severity = Severity::Error,
		"wrong option received"
	);
	let ResolvedValue::Channel(embed_channel) = channel_option.value else {
		bail!(
			"non-channel received for channel option (embed_channel; {:?}",
			channel_option.value
		);
	};

	ensure!(
		embed_channel.kind == ChannelType::Text,
		severity = Severity::Error,
		"wrong type of channel was entered for embed_channel ({:?})",
		embed_channel.kind
	);
	let embed_channel = embed_channel.id.to_channel(&ctx.http).await.into_diagnostic()?;
	let Some(embed_channel) = embed_channel.guild() else {
		bail!("Embed channel selected during setup is not in a guild");
	};
	if embed_channel.guild_id != guild {
		let message = CreateInteractionResponseMessage::new()
			.content("The channel you specified isn't in this server.")
			.ephemeral(true);
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let db_connection = get_database_connection(ctx).await;
	let mut db_connection = db_connection.lock().await;
	let new_guild_settings = GuildSettings {
		guild_id: guild.get() as i64,
		publish_channel: embed_channel.id.get() as i64,
		partner_role: None,
	};
	let insert_result = diesel::insert_into(guild_settings::table)
		.values(new_guild_settings)
		.execute(&mut *db_connection);
	let message = match insert_result {
		Ok(_) => CreateInteractionResponseMessage::new().content(format!("Initial setup complete! Once fully configured, the partnership embed will be published to <#{}>.", embed_channel.id.get())),
		Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => CreateInteractionResponseMessage::new().ephemeral(true).content("This server has already been set up. See `/settings` to modify individual settings or `/help` for other configuration."),
		Err(error) => bail!(error)
	};
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
