use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::PartnerCategory;
use crate::schema::partner_categories;
use crate::utils::guild_setup_check_with_reply;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;

/// Lists partner categories that have been created for this server
#[poise::command(slash_command, guild_only)]
pub async fn list(ctx: Context<'_>) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let sql_guild_id = guild.get() as i64;

	let mut db_connection = ctx.data().db_connection.lock().await;
	if !guild_setup_check_with_reply(ctx, guild, &mut db_connection).await? {
		return Ok(());
	}

	let categories: Vec<PartnerCategory> = partner_categories::table
		.filter(partner_categories::guild_id.eq(sql_guild_id))
		.load(&mut *db_connection)
		.into_diagnostic()?;

	let visible_categories: Vec<String> = categories
		.iter()
		.map(|category| format!("- {}", category.name))
		.collect();

	let message = format!(
		"The following partner categories have been set up:\n\n{}",
		visible_categories.join("\n")
	);

	let mut reply = CreateReply::default();
	reply = reply.ephemeral(true);
	reply = reply.content(message);
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
