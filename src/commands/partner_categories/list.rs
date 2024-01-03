use crate::database::get_database_connection;
use crate::schema::partner_categories;
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use miette::{bail, IntoDiagnostic};
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::client::Context;
use serenity::model::application::CommandInteraction;

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Partner categories list command was run outside of a guild context");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let mut db_connection = db_connection.lock().await;
	if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
		return Ok(());
	}

	let category_names: Vec<String> = partner_categories::table
		.filter(partner_categories::guild_id.eq(sql_guild_id))
		.select(partner_categories::name)
		.load(&mut *db_connection)
		.into_diagnostic()?;
	let mut message_lines = vec![String::from("The following partner categories have been set up:")];
	for name in category_names {
		message_lines.push(format!("- {}", name));
	}

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content(message_lines.join("\n"));
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
