use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::PartnerUser;
use crate::schema::{guild_settings, partner_users, partners};
use crate::utils::autocomplete::partner_display_name;
use crate::utils::GUILD_NOT_SET_UP;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DbError};
use miette::IntoDiagnostic;
use poise::reply::CreateReply;
use serenity::http::HttpError;
use serenity::model::guild::Guild;
use serenity::model::id::UserId;
use serenity::prelude::SerenityError;

/// Adds a user as a partner representative for a partner server
#[poise::command(slash_command, guild_only)]
pub async fn add_rep(
	ctx: Context<'_>,
	#[description = "The partner server for which to add the representative"]
	#[autocomplete = "partner_display_name"]
	partner_display_name: String,
	#[description = "The user to add as a representative"] user: UserId,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let mut db_connection = ctx.data().db_connection.lock().await;
	let sql_guild_id = guild.get() as i64;

	let partner_role_id: Option<Option<i64>> = guild_settings::table
		.find(sql_guild_id)
		.select(guild_settings::partner_role)
		.first(&mut *db_connection)
		.optional()
		.into_diagnostic()?;
	let partner_role_id = match partner_role_id {
		Some(id) => id.map(|id| id as u64),
		None => {
			let mut reply = CreateReply::default();
			reply = reply.ephemeral(true);
			reply = reply.content(GUILD_NOT_SET_UP);
			ctx.send(reply).await.into_diagnostic()?;
			return Ok(());
		}
	};

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
	let Some(partnership_id) = partnership_id else {
		let mut reply = CreateReply::default();
		reply = reply.ephemeral(true);
		reply = reply.content(format!("You have no partner named `{}`.", partner_display_name));
		ctx.send(reply).await.into_diagnostic()?;
		return Ok(());
	};

	let partner_user = PartnerUser {
		partnership_id,
		user_id: user.get() as i64,
	};
	let insert_result = diesel::insert_into(partner_users::table)
		.values(partner_user)
		.execute(&mut *db_connection);

	let mut complain_about_role_permissions = false;
	if let Some(partner_role_id) = partner_role_id {
		let guild_data = Guild::get(ctx, guild).await.into_diagnostic()?;
		let member = guild_data.member(ctx, user).await.into_diagnostic()?;
		if !member.roles.iter().any(|role| role.get() == partner_role_id) {
			let add_role_result = member.add_role(ctx, partner_role_id).await;
			if let Err(SerenityError::Http(HttpError::UnsuccessfulRequest(response))) = &add_role_result {
				if response.status_code.as_u16() == 403 {
					complain_about_role_permissions = true;
				} else {
					add_role_result.into_diagnostic()?
				}
			} else {
				add_role_result.into_diagnostic()?
			}
		}
	}

	let mut reply = CreateReply::default();
	let mut reply_content = match insert_result {
		Ok(_) => {
			format!("Added <@{}> as a partner for `{}`.", user.get(), partner_display_name)
		}
		Err(DbError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
			reply = reply.ephemeral(true);
			format!("<@{}> is already a partner for `{}`.", user.get(), partner_display_name)
		}
		Err(error) => Err(error).into_diagnostic()?,
	};
	if complain_about_role_permissions {
		reply_content = format!("{}\n**The bot does not have the correct permissions to update partner roles. You will need to add the partner role manually.**", reply_content);
	}
	reply = reply.content(reply_content);
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
