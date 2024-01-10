use crate::database::get_database_connection;
use crate::models::GuildSettings;
use crate::schema::{guild_settings, partner_users, partners};
use diesel::prelude::*;
use miette::IntoDiagnostic;
use serenity::client::Context;
use serenity::futures::StreamExt;
use serenity::model::id::{GuildId, RoleId, UserId};
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::interval;

pub async fn sync_role_for_guild(ctx: &Context, guild: GuildId, role: RoleId) -> miette::Result<()> {
	let db_connection = get_database_connection(ctx).await;
	let sql_guild_id = guild.get() as i64;
	let guild_partners: HashSet<UserId> = {
		let mut db_connection = db_connection.lock().await;
		let partners: Vec<i64> = partner_users::table
			.filter(
				partner_users::partnership_id.eq_any(
					partners::table
						.filter(partners::guild.eq(sql_guild_id))
						.select(partners::partnership_id),
				),
			)
			.select(partner_users::user_id)
			.distinct()
			.load(&mut *db_connection)
			.into_diagnostic()?;
		partners.into_iter().map(|id| UserId::new(id as u64)).collect()
	};

	let mut members = guild.members_iter(&ctx.http).boxed();
	while let Some(member) = members.next().await {
		let member = member.into_diagnostic()?;
		let is_partner = guild_partners.contains(&member.user.id);
		let has_partner_role = member.roles.iter().any(|role_id| *role_id == role);
		if is_partner != has_partner_role {
			if is_partner {
				member.add_role(&ctx.http, role).await.into_diagnostic()?;
			} else {
				member.remove_role(&ctx.http, role).await.into_diagnostic()?;
			}
		}
	}

	Ok(())
}

pub async fn sync_all_roles_task(ctx: &Context) -> miette::Result<()> {
	let mut interval = interval(Duration::from_secs(21600));
	let db_connection = get_database_connection(ctx).await;

	loop {
		interval.tick().await;

		let guilds_with_roles: Vec<GuildSettings> = {
			let mut db_connection = db_connection.lock().await;
			guild_settings::table
				.filter(guild_settings::partner_role.is_not_null())
				.load(&mut *db_connection)
				.into_diagnostic()?
		};

		for guild_data in guilds_with_roles {
			let guild = GuildId::new(guild_data.guild_id as u64);
			let role = RoleId::new(guild_data.partner_role.unwrap() as u64);
			let _ = sync_role_for_guild(ctx, guild, role).await; // Ignore permissions issues
		}
	}
}
