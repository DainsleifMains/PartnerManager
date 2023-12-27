use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::schema::{partner_categories, partners};
use crate::utils::autocomplete::{category_name, partner_display_name};
use diesel::prelude::*;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;

/// Changes the category a partner is in
#[poise::command(slash_command, guild_only)]
pub async fn set_category(
	ctx: Context<'_>,
	#[description = "The name of the partner"]
	#[autocomplete = "partner_display_name"]
	partner_display_name: String,
	#[description = "Partner category in which this partner should be"]
	#[autocomplete = "category_name"]
	new_partner_category: String,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let mut db_connection = ctx.data().db_connection.lock().await;

	let sql_guild_id = guild.get() as i64;

	let partnership_id: Option<String> = partners::table
		.filter(
			partners::guild
				.eq(sql_guild_id)
				.and(partners::display_name.eq(&partner_display_name)),
		)
		.select(partners::partnership_id)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let partnership_id = match partnership_id {
		Some(id) => id,
		None => {
			let mut reply = CreateReply::default();
			reply = reply.ephemeral(true);
			reply = reply.content(format!("You have no partner named `{}`.", partner_display_name));
			ctx.send(reply).await.into_diagnostic()?;
			return Ok(());
		}
	};

	let new_partner_category_id: Option<String> = partner_categories::table
		.filter(
			partner_categories::guild_id
				.eq(sql_guild_id)
				.and(partner_categories::name.eq(&new_partner_category)),
		)
		.select(partner_categories::id)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let new_partner_category_id = match new_partner_category_id {
		Some(id) => id,
		None => {
			let mut reply = CreateReply::default();
			reply = reply.ephemeral(true);
			reply = reply.content(format!(
				"You have no partner category named `{}`.",
				new_partner_category
			));
			ctx.send(reply).await.into_diagnostic()?;
			return Ok(());
		}
	};

	diesel::update(partners::table)
		.filter(partners::partnership_id.eq(&partnership_id))
		.set(partners::category.eq(&new_partner_category_id))
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	let mut reply = CreateReply::default();
	reply = reply.content(format!(
		"Updated the category of `{}` to `{}`.",
		partner_display_name, new_partner_category
	));
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
