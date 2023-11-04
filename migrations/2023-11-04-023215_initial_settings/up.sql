CREATE TABLE guild_settings (
	guild_id BIGINT PRIMARY KEY,
	publish_channel BIGINT NOT NULL,
	partner_role BIGINT
);

CREATE TABLE partner_categories (
	id TEXT PRIMARY KEY,
	guild_id BIGINT NOT NULL REFERENCES guild_settings,
	name TEXT NOT NULL
);

CREATE TABLE embed_data (
	guild BIGINT NOT NULL REFERENCES guild_settings,
	embed_part_sequence_number INTEGER NOT NULL,
	partner_category_list TEXT REFERENCES partner_categories,
	embed_text TEXT,
	PRIMARY KEY (guild, embed_part_sequence_number),
	CONSTRAINT has_embed_data CHECK(
		(partner_category_list IS NOT NULL OR embed_text IS NOT NULL)
		AND NOT
		(partner_category_list IS NOT NULL AND embed_text IS NOT NULL)
	)
);

CREATE TABLE partners (
	partnership_id TEXT PRIMARY KEY,
	guild BIGINT NOT NULL REFERENCES guild_settings,
	partner_guild BIGINT NOT NULL,
	partner_invite_link TEXT NOT NULL,
	CONSTRAINT unique_partner_guild UNIQUE (guild, partner_guild)
);

CREATE TABLE partner_users (
	partnership_id TEXT NOT NULL REFERENCES partners ON DELETE CASCADE,
	user_id BIGINT NOT NULL,
	PRIMARY KEY (partnership_id, user_id)
);