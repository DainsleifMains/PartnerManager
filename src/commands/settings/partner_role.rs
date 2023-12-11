use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::GuildSettings;
use crate::schema::guild_settings;
use crate::standard_replies::GUILD_NOT_SET_UP;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;
use serenity::model::guild::Role;

/// Manages the role partners should have
#[poise::command(slash_command, guild_only, subcommands("get", "set", "remove"))]
pub async fn partner_role(_ctx: Context<'_>) -> Result<(), CommandError> {
	Err(CommandErrorValue::BadParentCommand)?
}

/// Gets the role assigned to all partners
#[poise::command(slash_command, guild_only)]
async fn get(ctx: Context<'_>) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let sql_guild_id = guild.get() as i64;
	let mut db_connection = ctx.data().db_connection.lock().await;

	let role: Option<i64> = guild_settings::table
		.find(sql_guild_id)
		.select(guild_settings::partner_role)
		.first(&mut *db_connection)
		.into_diagnostic()?;
	let role = role.map(|id| id as u64);

	let mut reply = CreateReply::default();
	if let Some(role_id) = role {
		reply = reply.content(format!("The current partner role is <@{}>.", role_id));
	} else {
		reply = reply.content("There is no partner role.");
	}
	reply = reply.ephemeral(true);
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn set(ctx: Context<'_>, partner_role: Role) -> Result<(), CommandError> {
	update_role(ctx, Some(partner_role)).await
}

#[poise::command(slash_command, guild_only)]
async fn remove(ctx: Context<'_>) -> Result<(), CommandError> {
	update_role(ctx, None).await
}

async fn update_role(ctx: Context<'_>, partner_role: Option<Role>) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	if let Some(role) = partner_role.as_ref() {
		if role.guild_id != guild {
			Err(CommandErrorValue::WrongGuild)?
		}
	}

	let sql_guild_id = guild.get() as i64;
	let sql_role_id = partner_role.as_ref().map(|role| role.id.get() as i64);

	let mut db_connection = ctx.data().db_connection.lock().await;

	let updated_settings: Option<GuildSettings> = diesel::update(guild_settings::table)
		.filter(guild_settings::guild_id.eq(sql_guild_id))
		.set(guild_settings::partner_role.eq(sql_role_id))
		.get_result(&mut *db_connection)
		.optional()
		.into_diagnostic()?;

	// TODO: Update the role assigned to partner users

	match updated_settings {
		Some(_) => {
			let message = match partner_role.as_ref() {
				Some(role) => format!("Updated the partner role to <@&{}>.", role.id.get()),
				None => String::from("Removed partner role."),
			};
			ctx.send(CreateReply::default().content(message))
				.await
				.into_diagnostic()?;
		}
		None => {
			ctx.send(CreateReply::default().content(GUILD_NOT_SET_UP).ephemeral(true))
				.await
				.into_diagnostic()?;
		}
	}

	Ok(())
}
