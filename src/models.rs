use crate::schema::{embed_data, guild_settings, partner_categories, partner_users, partners};
use diesel::prelude::*;

#[derive(Insertable, Queryable)]
#[diesel(table_name = guild_settings)]
pub struct GuildSettings {
	pub guild_id: i64,
	pub publish_channel: i64,
	pub published_message_id: Option<i64>,
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
	pub guild: i64,
	pub embed_part_sequence_number: i32,
	pub partner_category_list: Option<String>,
	pub embed_text: String,
	pub image_url: String,
	pub title: String,
	pub author: String,
	pub footer: String,
	pub color: Option<i32>,
}

#[derive(Insertable, Queryable)]
pub struct Partner {
	pub partnership_id: String,
	pub guild: i64,
	pub partner_guild: i64,
	pub partner_invite_link: String,
}

#[derive(Insertable, Queryable)]
pub struct PartnerUser {
	pub partnership_id: String,
	pub user_id: i64,
}
