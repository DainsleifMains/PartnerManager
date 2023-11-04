use crate::schema::{embed_data, guild_settings, partner_categories, partner_users, partners};
use diesel::prelude::*;

#[derive(Insertable, Queryable)]
#[diesel(table_name = guild_settings)]
pub struct GuildSettings {
	pub guild_id: i64,
	pub publish_channel: i64,
	pub partner_role: Option<i64>,
}

#[derive(Insertable, Queryable)]
#[diesel(table_name = partner_categories)]
pub struct PartnerCategory {
	pub id: String,
	pub guild_id: i64,
	pub name: String,
}

#[derive(Insertable, Queryable)]
#[diesel(table_name = embed_data)]
pub struct EmbedData {
	guild: i64,
	embed_part_sequence_number: i32,
	partner_category_list: Option<String>,
	embed_text: Option<String>,
}

#[derive(Insertable, Queryable)]
pub struct Partner {
	partnership_id: String,
	guild: i64,
	partner_guild: i64,
	partner_invite_link: String,
}

#[derive(Insertable, Queryable)]
pub struct PartnerUser {
	partnership_id: String,
	user_id: i64,
}
