use crate::database::get_database_connection;
use crate::models::{EmbedData, Partner, PublishedMessage};
use crate::schema::{embed_data, guild_settings, partners, published_messages};
use diesel::prelude::*;
use miette::IntoDiagnostic;
use serenity::builder::{CreateEmbed, CreateMessage, EditMessage};
use serenity::client::Context;
use serenity::model::id::{ChannelId, GuildId, MessageId};

pub async fn remove_embed(ctx: &Context, guild: GuildId) -> miette::Result<()> {
	let db_connection = get_database_connection(ctx).await;
	let sql_guild_id = guild.get() as i64;

	let (channel, messages) = {
		let mut db_connection = db_connection.lock().await;

		let messages: Vec<PublishedMessage> = published_messages::table
			.filter(published_messages::guild_id.eq(sql_guild_id))
			.load(&mut *db_connection)
			.into_diagnostic()?;
		let channel_id: i64 = guild_settings::table
			.find(sql_guild_id)
			.select(guild_settings::publish_channel)
			.first(&mut *db_connection)
			.into_diagnostic()?;

		(ChannelId::new(channel_id as u64), messages)
	};

	let mut successfully_removed_messages: Vec<u64> = Vec::new();
	for message in messages {
		let message_id = message.message_id as u64;

		// Ignore permission errors
		if channel.delete_message(&ctx.http, message_id).await.is_ok() {
			successfully_removed_messages.push(message_id);
		}
	}

	let delete_message: Vec<i64> = successfully_removed_messages.into_iter().map(|id| id as i64).collect();
	let mut db_connection = db_connection.lock().await;
	diesel::delete(published_messages::table)
		.filter(
			published_messages::guild_id
				.eq(sql_guild_id)
				.and(published_messages::message_id.eq_any(delete_message)),
		)
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	Ok(())
}

pub async fn update_embed(ctx: &Context, guild: GuildId) -> miette::Result<()> {
	let db_connection = get_database_connection(ctx).await;
	let sql_guild_id = guild.get() as i64;
	let mut db_connection = db_connection.lock().await;

	let channel_id: i64 = guild_settings::table
		.find(sql_guild_id)
		.select(guild_settings::publish_channel)
		.first(&mut *db_connection)
		.into_diagnostic()?;
	let channel_id = ChannelId::new(channel_id as u64);
	let existing_messages: Vec<PublishedMessage> = published_messages::table
		.filter(published_messages::guild_id.eq(sql_guild_id))
		.load(&mut *db_connection)
		.into_diagnostic()?;
	let embed_data: Vec<EmbedData> = embed_data::table
		.filter(embed_data::guild.eq(sql_guild_id))
		.order(embed_data::embed_part_sequence_number.asc())
		.load(&mut *db_connection)
		.into_diagnostic()?;

	let mut embeds: Vec<CreateEmbed> = Vec::with_capacity(embed_data.len());
	for embed in embed_data {
		let mut new_embed = CreateEmbed::new();
		if !embed.embed_text.is_empty() {
			new_embed = new_embed.description(embed.embed_text);
		}
		if !embed.image_url.is_empty() {
			new_embed = new_embed.image(embed.image_url);
		}
		if let Some(color_number) = embed.color {
			new_embed = new_embed.color(color_number);
		}

		if let Some(partner_category) = embed.partner_category_list {
			let partners: Vec<Partner> = partners::table
				.filter(partners::category.eq(&partner_category))
				.order(partners::display_name.asc())
				.load(&mut *db_connection)
				.into_diagnostic()?;

			if !partners.is_empty() {
				// To prevent potential problems with field contents length, limit fields to 10 partners each
				let mut field_lines: Vec<Vec<String>> = vec![Vec::new()];
				for partner in partners.iter() {
					let mut last_field = field_lines.last_mut().unwrap();
					if last_field.len() >= 10 {
						field_lines.push(Vec::new());
						last_field = field_lines.last_mut().unwrap();
					}
					last_field.push(format!(
						"- [{}](https://discord.gg/{})",
						partner.display_name, partner.invite_code
					));
				}

				let field_contents: Vec<String> = field_lines.into_iter().map(|lines| lines.join("\n")).collect();
				new_embed = new_embed.fields(field_contents.iter().map(|contents| ("", contents, true)));
			}
		}
		embeds.push(new_embed);
	}

	if embeds.is_empty() {
		if !existing_messages.is_empty() {
			for message in existing_messages {
				let message_id = message.message_id as u64;

				// Ignore permission errors
				let _ = channel_id.delete_message(&ctx.http, message_id).await;
			}
			diesel::delete(published_messages::table)
				.filter(published_messages::guild_id.eq(sql_guild_id))
				.execute(&mut *db_connection)
				.into_diagnostic()?;
		}
	} else {
		// Group embed into groups of 10
		// For now, we're assuming that embeds won't exceed Discord's limit of 6000 characters across all embeds in a message
		let mut embed_groups: Vec<Vec<CreateEmbed>> = vec![Vec::new()];
		for embed in embeds {
			let mut last_group = embed_groups.last_mut().unwrap();
			if last_group.len() >= 10 {
				embed_groups.push(Vec::new());
				last_group = embed_groups.last_mut().unwrap();
			}
			last_group.push(embed);
		}

		let mut new_message_ids: Vec<MessageId> = Vec::new();
		let mut existing_message_iter = existing_messages.into_iter();

		for embed_group in embed_groups {
			match existing_message_iter.next() {
				Some(message) => {
					let message_id = message.message_id as u64;
					let message = EditMessage::new().embeds(embed_group);
					channel_id
						.edit_message(&ctx.http, message_id, message)
						.await
						.into_diagnostic()?;
				}
				None => {
					let message = CreateMessage::new().embeds(embed_group);
					let new_message = channel_id.send_message(&ctx.http, message).await.into_diagnostic()?;
					new_message_ids.push(new_message.id);
				}
			}
		}

		let unused_existing_messages: Vec<i64> = existing_message_iter.map(|message| message.message_id).collect();
		if !unused_existing_messages.is_empty() {
			diesel::delete(published_messages::table)
				.filter(
					published_messages::guild_id
						.eq(sql_guild_id)
						.and(published_messages::message_id.eq_any(&unused_existing_messages)),
				)
				.execute(&mut *db_connection)
				.into_diagnostic()?;
		}

		let new_messages: Vec<PublishedMessage> = new_message_ids
			.into_iter()
			.map(|message_id| PublishedMessage {
				guild_id: sql_guild_id,
				message_id: message_id.get() as i64,
			})
			.collect();
		if !new_messages.is_empty() {
			diesel::insert_into(published_messages::table)
				.values(new_messages)
				.execute(&mut *db_connection)
				.into_diagnostic()?;
		}
	}

	Ok(())
}
