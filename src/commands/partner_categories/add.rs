use crate::database::get_database_connection;
use crate::models::PartnerCategory;
use crate::schema::partner_categories;
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DbError};
use miette::{bail, ensure, IntoDiagnostic, Severity};
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::client::Context;
use serenity::model::application::{CommandInteraction, ResolvedOption, ResolvedValue};

pub async fn execute(
	ctx: &Context,
	command: &CommandInteraction,
	options: &[ResolvedOption<'_>],
) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Partner categories command was run outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let mut db_connection = db_connection.lock().await;
	if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
		return Ok(());
	}

	let Some(name_option) = options.first() else {
		bail!("Insufficient options passed to partner_categories add command");
	};
	ensure!(
		name_option.name == "name",
		severity = Severity::Error,
		"Incorrect options passed to parnter_categories add command"
	);
	let ResolvedValue::String(name) = name_option.value else {
		bail!(
			"Incorrect value type passed for name option of partner_categories add command: {:?}",
			name_option.value
		);
	};

	let new_category = PartnerCategory {
		id: cuid2::create_id(),
		guild_id: sql_guild_id,
		name: name.to_string(),
	};

	let insert_result = diesel::insert_into(partner_categories::table)
		.values(new_category)
		.execute(&mut *db_connection);

	let message = match insert_result {
		Ok(_) => CreateInteractionResponseMessage::new()
			.content(format!("Created new partner category with the name {}.", name)),
		Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content(format!("A category named {} already exists for this server.", name)),
		Err(error) => bail!(error),
	};
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
