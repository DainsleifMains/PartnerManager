use crate::database::get_database_connection;
use crate::schema::guild_settings;
use crate::utils::setup_check::GUILD_NOT_SET_UP;
use diesel::prelude::*;
use miette::{bail, ensure, IntoDiagnostic, Severity};
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::client::Context;
use serenity::model::application::CommandInteraction;
use serenity::model::application::{ResolvedOption, ResolvedValue};

pub async fn execute(
	ctx: &Context,
	command: &CommandInteraction,
	options: &[ResolvedOption<'_>],
) -> miette::Result<()> {
	let Some(option) = options.first() else {
		bail!("Insufficient subcommands passed to settings partner_role command");
	};
	let ResolvedValue::SubCommand(subcommand_options) = &option.value else {
		bail!("Incorrect data type passed for settings partner_role subcommand");
	};
	match option.name {
		"get" => get(ctx, command).await,
		"set" => set(ctx, command, subcommand_options).await,
		_ => bail!("Unexpected subcommand passed to settings partner_role: {}", option.name),
	}
}

async fn get(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Settings command was used outside of a guild");
	};
	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let mut db_connection = db_connection.lock().await;

	let role: Option<Option<i64>> = guild_settings::table
		.find(sql_guild_id)
		.select(guild_settings::partner_role)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let role = role.map(|id| id.map(|id| id as u64));

	let reply = match role {
		Some(Some(id)) => format!("The current partner role is <@&{}>.", id),
		Some(None) => String::from("There is no partner role."),
		None => GUILD_NOT_SET_UP.to_string(),
	};

	let message = CreateInteractionResponseMessage::new().ephemeral(true).content(reply);
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
	let role_option = options.first();

	let role = match role_option {
		Some(role_value) => {
			ensure!(
				role_value.name == "partner_role",
				severity = Severity::Error,
				"wrong option received by settings partner_role set command"
			);
			let ResolvedValue::Role(role) = role_value.value else {
				bail!("Got a non-role value for a role option");
			};
			Some(role)
		}
		None => None,
	};

	if let Some(role) = &role {
		if role.guild_id != guild {
			let message = CreateInteractionResponseMessage::new()
				.ephemeral(true)
				.content("The role you provided is for a different server.");
			command
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
			return Ok(());
		}
	}

	let sql_guild_id = guild.get() as i64;
	let sql_role_id = role.as_ref().map(|role| role.id.get() as i64);

	let db_connection = get_database_connection(ctx).await;
	let mut db_connection = db_connection.lock().await;

	let old_role: Option<Option<i64>> = guild_settings::table
		.find(sql_guild_id)
		.select(guild_settings::partner_role)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let old_role = match old_role {
		Some(role) => role,
		None => {
			let message = CreateInteractionResponseMessage::new()
				.ephemeral(true)
				.content(GUILD_NOT_SET_UP);
			command
				.create_response(&ctx.http, CreateInteractionResponse::Message(message))
				.await
				.into_diagnostic()?;
			return Ok(());
		}
	};

	diesel::update(guild_settings::table)
		.filter(guild_settings::guild_id.eq(sql_guild_id))
		.set(guild_settings::partner_role.eq(sql_role_id))
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	// TODO: Update the role assigned to partner users

	let message = match (role.as_ref(), old_role) {
		(Some(role), Some(old_role)) => CreateInteractionResponseMessage::new().content(format!(
			"Updated the partner role to <@&{}> (from <@&{}>).",
			role.id.get(),
			old_role
		)),
		(Some(role), None) => CreateInteractionResponseMessage::new()
			.content(format!("Updated the partner role to <@&{}>.", role.id.get())),
		(None, _) => CreateInteractionResponseMessage::new().content("Removed partner role."),
	};
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
