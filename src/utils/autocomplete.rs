use crate::command_types::Context;
use crate::schema::partners;
use diesel::prelude::*;
use futures::Stream;

pub async fn partner_display_name(ctx: Context<'_>, partial: &str) -> impl Stream<Item = String> {
	let Some(guild) = ctx.guild_id() else {
		return futures::stream::iter(Vec::new());
	};

	let sql_guild_id = guild.get() as i64;
	let mut db_connection = ctx.data().db_connection.lock().await;

	let search_results: QueryResult<Vec<String>> = partners::table
		.filter(
			partners::guild
				.eq(sql_guild_id)
				.and(partners::display_name.like(format!("{}%", partial))),
		)
		.select(partners::display_name)
		.load(&mut *db_connection);
	futures::stream::iter(search_results.unwrap_or_default())
}
