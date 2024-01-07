use crate::schema::{embed_data, guild_settings, partner_categories, partner_users, partners, published_messages};
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

#[derive(Clone, Insertable, Queryable)]
#[diesel(table_name = embed_data)]
pub struct EmbedData {
	pub id: String,
	pub guild: i64,
	pub embed_part_sequence_number: i32,
	pub embed_name: String,
	pub partner_category_list: Option<String>,
	pub embed_text: String,
	pub image_url: String,
	pub color: Option<i32>,
}

#[derive(Insertable, Queryable)]
pub struct Partner {
	pub partnership_id: String,
	pub guild: i64,
	pub category: String,
	pub partner_guild: i64,
	pub display_name: String,
	pub invite_code: String,
}

#[derive(Insertable, Queryable)]
pub struct PartnerUser {
	pub partnership_id: String,
	pub user_id: i64,
}

#[derive(Insertable, Queryable)]
pub struct PublishedMessage {
	pub guild_id: i64,
	pub message_id: i64,
}
