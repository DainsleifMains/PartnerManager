// @generated automatically by Diesel CLI.

diesel::table! {
	embed_data (id) {
		id -> Text,
		guild -> Int8,
		embed_part_sequence_number -> Int4,
		embed_name -> Text,
		partner_category_list -> Nullable<Text>,
		embed_text -> Text,
		image_url -> Text,
		color -> Nullable<Int4>,
	}
}

diesel::table! {
	guild_settings (guild_id) {
		guild_id -> Int8,
		publish_channel -> Int8,
		partner_role -> Nullable<Int8>,
	}
}

diesel::table! {
	partner_categories (id) {
		id -> Text,
		guild_id -> Int8,
		name -> Text,
	}
}

diesel::table! {
	partner_users (partnership_id, user_id) {
		partnership_id -> Text,
		user_id -> Int8,
	}
}

diesel::table! {
	partners (partnership_id) {
		partnership_id -> Text,
		guild -> Int8,
		category -> Text,
		partner_guild -> Int8,
		display_name -> Text,
		partner_invite_link -> Text,
	}
}

diesel::table! {
	published_messages (guild_id, message_id) {
		guild_id -> Int8,
		message_id -> Int8,
	}
}

diesel::joinable!(embed_data -> guild_settings (guild));
diesel::joinable!(embed_data -> partner_categories (partner_category_list));
diesel::joinable!(partner_categories -> guild_settings (guild_id));
diesel::joinable!(partner_users -> partners (partnership_id));
diesel::joinable!(partners -> guild_settings (guild));
diesel::joinable!(partners -> partner_categories (category));
diesel::joinable!(published_messages -> guild_settings (guild_id));

diesel::allow_tables_to_appear_in_same_query!(
	embed_data,
	guild_settings,
	partner_categories,
	partner_users,
	partners,
	published_messages,
);
