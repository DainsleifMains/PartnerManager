CREATE TABLE guild_settings (
	guild_id BIGINT PRIMARY KEY,
	publish_channel BIGINT NOT NULL,
	partner_role BIGINT
);

CREATE TABLE partner_categories (
	id TEXT PRIMARY KEY,
	guild_id BIGINT NOT NULL REFERENCES guild_settings,
	name TEXT NOT NULL,
	CONSTRAINT unique_category_per_guild UNIQUE (guild_id, name)
);

CREATE TABLE embed_data (
	id TEXT PRIMARY KEY,
	guild BIGINT NOT NULL REFERENCES guild_settings,
	embed_part_sequence_number INTEGER NOT NULL,
	embed_name TEXT NOT NULL,
	partner_category_list TEXT REFERENCES partner_categories,
	embed_text TEXT NOT NULL,
	image_url TEXT NOT NULL,
	color INTEGER,
	CONSTRAINT unique_index_per_guild UNIQUE (guild, embed_part_sequence_number),
	CONSTRAINT unique_embed_name_per_guild UNIQUE (guild, embed_name)
);

CREATE TABLE partners (
	partnership_id TEXT PRIMARY KEY,
	guild BIGINT NOT NULL REFERENCES guild_settings,
	category TEXT NOT NULL REFERENCES partner_categories,
	partner_guild BIGINT NOT NULL,
	display_name TEXT NOT NULL,
	partner_invite_link TEXT NOT NULL,
	CONSTRAINT unique_partner_guild UNIQUE (guild, partner_guild),
	CONSTRAINT unique_partner_display_name UNIQUE (guild, display_name)
);

CREATE TABLE partner_users (
	partnership_id TEXT NOT NULL REFERENCES partners ON DELETE CASCADE,
	user_id BIGINT NOT NULL,
	PRIMARY KEY (partnership_id, user_id)
);

CREATE TABLE published_messages (
	guild_id BIGINT NOT NULL REFERENCES guild_settings,
	message_id BIGINT NOT NULL,
	PRIMARY KEY (guild_id, message_id)
);