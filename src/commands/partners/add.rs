use crate::command_types::{CommandError, CommandErrorValue, Context};
use crate::models::{Partner, PartnerCategory};
use crate::schema::{partner_categories, partners};
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind;
use futures::Stream;
use miette::IntoDiagnostic;
use poise::reply::CreateReply;
use serenity::model::invite::Invite;
use serenity::utils::parse_invite;

async fn autocomplete_category_name(ctx: Context<'_>, partial: &str) -> impl Stream<Item = String> {
	let Some(guild) = ctx.guild_id() else {
		return futures::stream::iter(Vec::new());
	};

	let sql_guild_id = guild.get() as i64;
	let mut db_connection = ctx.data().db_connection.lock().await;

	let search_results: QueryResult<Vec<String>> = partner_categories::table
		.filter(
			partner_categories::guild_id
				.eq(sql_guild_id)
				.and(partner_categories::name.like(format!("{}%", partial))),
		)
		.select(partner_categories::name)
		.load(&mut *db_connection);
	futures::stream::iter(search_results.unwrap_or_default())
}

/// Adds a new partner server to the partner list
#[poise::command(slash_command, guild_only)]
pub async fn add(
	ctx: Context<'_>,
	#[description = "The partner category to which to add the new partner"]
	#[autocomplete = "autocomplete_category_name"]
	category_name: String,
	#[description = "A permanent invite link for the guild"] invite_link: String,
	#[description = "Display name for the server; defaults to the server name"] display_name: Option<String>,
) -> Result<(), CommandError> {
	let Some(guild) = ctx.guild_id() else {
		Err(CommandErrorValue::GuildExpected)?
	};

	let sql_guild_id = guild.get() as i64;

	let invite_code = parse_invite(&invite_link);
	let invite = match Invite::get(ctx, invite_code, false, true, None).await {
		Ok(invite) => invite,
		Err(_) => {
			let mut reply = CreateReply::default();
			reply = reply.ephemeral(true);
			reply = reply.content("The invite link is invalid.");
			ctx.send(reply).await.into_diagnostic()?;
			return Ok(());
		}
	};

	let partner_guild = match invite.guild {
		Some(invite_guild) => invite_guild,
		None => {
			let mut reply = CreateReply::default();
			reply = reply.ephemeral(true);
			reply = reply.content("The invite link is invalid; could not retrieve a server for it.");
			ctx.send(reply).await.into_diagnostic()?;
			return Ok(());
		}
	};

	if invite.expires_at.is_some() {
		let mut reply = CreateReply::default();
		reply = reply.ephemeral(true);
		reply = reply.content("The invite link is not permanent.");
		ctx.send(reply).await.into_diagnostic()?;
		return Ok(());
	}

	let partner_guild_name = match display_name {
		Some(name) => name,
		None => partner_guild.name,
	};
	let partner_guild = partner_guild.id;

	let mut db_connection = ctx.data().db_connection.lock().await;

	let category: QueryResult<PartnerCategory> = partner_categories::table
		.filter(
			partner_categories::guild_id
				.eq(sql_guild_id)
				.and(partner_categories::name.eq(&category_name)),
		)
		.first(&mut *db_connection);
	let category = match category {
		Ok(category) => category,
		Err(_) => {
			let mut reply = CreateReply::default();
			reply = reply.ephemeral(true);
			reply = reply.content(format!("The partner category `{}` doesn't exist.", category_name));
			ctx.send(reply).await.into_diagnostic()?;
			return Ok(());
		}
	};

	let new_partner = Partner {
		partnership_id: cuid2::create_id(),
		guild: sql_guild_id,
		category: category.id,
		partner_guild: partner_guild.get() as i64,
		display_name: partner_guild_name.clone(),
		partner_invite_link: invite_link.clone(),
	};
	let insert_result: QueryResult<_> = diesel::insert_into(partners::table)
		.values(new_partner)
		.execute(&mut *db_connection);
	if let Err(error) = insert_result {
		let mut reply = CreateReply::default();
		reply = reply.ephemeral(true);
		reply = reply.content(format!("Failed to add the partner ({})", error));
		ctx.send(reply).await.into_diagnostic()?;
		return Ok(());
	}

	let mut reply = CreateReply::default();
	match insert_result {
		Ok(_) => reply = reply.content(format!("Added [{}]({}) as a partner!", partner_guild_name, invite_link)),
		Err(diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, violation_info)) => {
			reply = reply.ephemeral(true);
			match violation_info.constraint_name() {
				Some("unique_partner_guild") => reply = reply.content("That server is already a partner."),
				Some("unique_partner_display_name") => {
					reply = reply.content("That display name is already in use for another server.")
				}
				_ => reply = reply.content("An unknown collision with another partnership occurred."),
			}
		}
		Err(error) => reply = reply.content(format!("Failed to add the partner ({})", error)),
	}
	ctx.send(reply).await.into_diagnostic()?;

	Ok(())
}
