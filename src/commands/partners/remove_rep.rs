use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::schema::{guild_settings, partner_users, partners};
use crate::utils::autocomplete::partner_display_name;
use crate::utils::GUILD_NOT_SET_UP;
use diesel::dsl::count_star;
use diesel::prelude::*;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;
use serenity::http::{ErrorResponse, HttpError, StatusCode};
use serenity::model::guild::Guild;
use serenity::model::id::UserId;
use serenity::prelude::SerenityError;

/// Removes a representative for a partner
#[poise::command(slash_command, guild_only)]
pub async fn remove_rep(
	ctx: Context<'_>,
	#[description = "The partner for which to remove the representative"]
	#[autocomplete = "partner_display_name"]
	partner_display_name: String,
	#[description = "The user to remove as a representative"] user: UserId,
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

	let sql_user_id = user.get() as i64;
	let delete_count = diesel::delete(partner_users::table)
		.filter(
			partner_users::partnership_id
				.eq(partnership_id)
				.and(partner_users::user_id.eq(sql_user_id)),
		)
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	let mut complain_about_role_permissions = false;
	if let Some(partner_role_id) = partner_role_id {
		let partners_represented: i64 = partner_users::table
			.filter(
				partner_users::user_id.eq(sql_user_id).and(
					partner_users::partnership_id.eq_any(
						partners::table
							.filter(partners::guild.eq(sql_guild_id))
							.select(partners::partnership_id),
					),
				),
			)
			.select(count_star())
			.first(&mut *db_connection)
			.into_diagnostic()?;
		if partners_represented == 0 {
			let guild_data = Guild::get(ctx, guild).await.into_diagnostic()?;
			let member = guild_data.member(ctx, user).await;
			match member {
				Ok(member) => {
					if member.roles.iter().any(|role| role.get() == partner_role_id) {
						let remove_role_result = member.remove_role(ctx, partner_role_id).await;
						if let Err(SerenityError::Http(HttpError::UnsuccessfulRequest(ErrorResponse {
							status_code: StatusCode::FORBIDDEN,
							..
						}))) = &remove_role_result
						{
							complain_about_role_permissions = true;
						} else {
							remove_role_result.into_diagnostic()?
						}
					}
				}
				Err(error) => {
					match error {
						SerenityError::Http(HttpError::UnsuccessfulRequest(ErrorResponse {
							status_code: StatusCode::NOT_FOUND,
							..
						})) => (), // If 404, user is no longer a member, so just ignore
						_ => Err(error).into_diagnostic()?,
					}
				}
			}
		}
	}

	let mut reply = CreateReply::default();
	let mut reply_content = if delete_count == 0 {
		reply = reply.ephemeral(true);
		format!(
			"<@{}> wasn't a partner representative for `{}`.",
			user.get(),
			partner_display_name
		)
	} else {
		format!("Removed <@{}> as a partner for `{}`.", user.get(), partner_display_name)
	};
	if complain_about_role_permissions {
		reply_content = format!("{}\n**The bot does not have the correct permissions to update partner roles. You will need to remove the partner role manually.**", reply_content);
	}
	reply = reply.content(reply_content);
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
