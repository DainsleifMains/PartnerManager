use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::PartnerCategory;
use crate::schema::partner_categories;
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;

/// Adds a partnership category with the provided name
#[poise::command(slash_command, guild_only)]
pub async fn add(
	ctx: Context<'_>,
	#[description = "The name to give the new category"] name: String,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let sql_guild_id = guild.get() as i64;

	let mut db_connection = ctx.data().db_connection.lock().await;

	let new_category_id = cuid2::create_id();
	let new_category = PartnerCategory {
		id: new_category_id,
		guild_id: sql_guild_id,
		name: name.clone(),
	};

	let insert_result = diesel::insert_into(partner_categories::table)
		.values(new_category)
		.execute(&mut *db_connection);

	let mut reply = CreateReply::default();
	match insert_result {
		Ok(_) => {
			reply = reply.content(format!("Created new partner category with the name `{}`.", name));
		}
		Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			reply = reply.ephemeral(true);
			reply = reply.content("A category with that name already exists for this server.");
		}
		Err(error) => Err(error).into_diagnostic()?,
	};
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
