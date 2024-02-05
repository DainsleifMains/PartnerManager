CREATE TABLE partner_self_users (
	partnership TEXT NOT NULL REFERENCES partners,
	user_id BIGINT NOT NULL,
	PRIMARY KEY (partnership, user_id)
);