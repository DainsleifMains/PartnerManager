use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::schema::partners;
use crate::utils::autocomplete::partner_display_name;
use crate::utils::guild_setup_check_with_reply;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DbError};
use miette::IntoDiagnostic;
use poise::reply::CreateReply;

/// Changes the display name of a partner server
#[poise::command(slash_command, guild_only)]
pub async fn set_name(
	ctx: Context<'_>,
	#[description = "The name of the server to change"]
	#[autocomplete = "partner_display_name"]
	partner_display_name: String,
	#[description = "The new name to use"] new_display_name: String,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let mut db_connection = ctx.data().db_connection.lock().await;
	if !guild_setup_check_with_reply(ctx, guild, &mut db_connection).await? {
		return Ok(());
	}

	let sql_guild_id = guild.get() as i64;

	let update_result: QueryResult<()> = db_connection.transaction(|db_connection| {
		let partner_id: String = partners::table
			.filter(
				partners::guild
					.eq(sql_guild_id)
					.and(partners::display_name.eq(&partner_display_name)),
			)
			.select(partners::partnership_id)
			.first(db_connection)?;
		diesel::update(partners::table)
			.filter(partners::partnership_id.eq(&partner_id))
			.set(partners::display_name.eq(&new_display_name))
			.execute(db_connection)?;

		Ok(())
	});

	// TODO: embed update

	let mut reply = CreateReply::default();
	match update_result {
		Ok(()) => {
			reply = reply.content(format!(
				"The name of `{}` has been updated to `{}`.",
				partner_display_name, new_display_name
			))
		}
		Err(DbError::NotFound) => {
			reply = reply.ephemeral(true);
			reply = reply.content(format!("You have no partner with the name `{}`.", partner_display_name));
		}
		Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			reply = reply.ephemeral(true);
			reply = reply.content(format!(
				"You already have another partner named `{}`.",
				new_display_name
			));
		}
		Err(_) => update_result.into_diagnostic()?,
	}
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
