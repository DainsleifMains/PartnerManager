use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::schema::partner_categories;
use crate::utils::guild_setup_check_with_reply;
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind;
use futures::Stream;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;

async fn autocomplete_name(ctx: Context<'_>, partial: &str) -> impl Stream<Item = String> {
	let Some(guild) = ctx.guild_id() else {
		return futures::stream::iter(Vec::new());
	};

	let sql_guild_id = guild.get() as i64;
	let name_like = format!("{}%", partial);
	let mut db_connection = ctx.data().db_connection.lock().await;

	let names: Vec<String> = partner_categories::table
		.filter(
			partner_categories::guild_id
				.eq(sql_guild_id)
				.and(partner_categories::name.like(name_like)),
		)
		.select(partner_categories::name)
		.load(&mut *db_connection)
		.unwrap_or_default();

	futures::stream::iter(names)
}

/// Deletes a partnership category with the given name
///
/// Deletes a partnership category for your server with a name matching the one provided with this command. Categories
/// must be empty (have no partners) before they can be removed.
#[poise::command(slash_command, guild_only)]
pub async fn remove(
	ctx: Context<'_>,
	#[description = "The name of the category to remove"]
	#[autocomplete = "autocomplete_name"]
	name: String,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let sql_guild_id = guild.get() as i64;
	let mut db_connection = ctx.data().db_connection.lock().await;
	if !guild_setup_check_with_reply(ctx, guild, &mut db_connection).await? {
		return Ok(());
	}

	let delete_result = diesel::delete(partner_categories::table)
		.filter(
			partner_categories::guild_id
				.eq(sql_guild_id)
				.and(partner_categories::name.eq(&name)),
		)
		.execute(&mut *db_connection);

	let mut reply = CreateReply::default();
	match delete_result {
		Ok(0) => {
			reply = reply.ephemeral(true);
			reply = reply.content("No category with that name exists.");
		}
		Ok(_) => {
			reply = reply.content(format!("Deleted the partner category `{}`.", name));
		}
		Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _)) => {
			reply = reply.ephemeral(true);
			reply = reply.content(format!("`{}` cannot be removed because it is in use.", name));
		}
		Err(error) => Err(error).into_diagnostic()?,
	}
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
