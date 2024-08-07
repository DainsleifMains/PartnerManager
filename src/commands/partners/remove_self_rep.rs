use crate::database::get_database_connection;
use crate::models::Partner;
use crate::schema::{partner_self_users, partners};
use crate::utils::pagination::{get_partners_for_page, max_partner_page};
use crate::utils::setup_check::guild_setup_check_with_reply;
use diesel::prelude::*;
use miette::{bail, IntoDiagnostic};
use serenity::builder::{
	CreateActionRow, CreateAllowedMentions, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
	CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse,
};
use serenity::client::Context;
use serenity::collector::ComponentInteractionCollector;
use serenity::http::{ErrorResponse, HttpError, StatusCode};
use serenity::model::application::{
	ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionDataKind,
};
use serenity::model::id::UserId;
use serenity::prelude::SerenityError;
use std::time::Duration;

struct ComponentsData<'a> {
	partners: &'a [Partner],
	reps: &'a [(u64, String)],
	current_partner_page: usize,
	current_partner_id: &'a str,
	current_user_id: &'a str,
	partner_select_id: &'a str,
	rep_select_id: &'a str,
	submit_button_id: &'a str,
	cancel_button_id: &'a str,
}

fn components_to_display(component_data: ComponentsData) -> Vec<CreateActionRow> {
	let ComponentsData {
		partners,
		reps,
		current_partner_page,
		current_partner_id,
		current_user_id,
		partner_select_id,
		rep_select_id,
		submit_button_id,
		cancel_button_id,
	} = component_data;

	let partner_select_options = get_partners_for_page(partners, current_partner_page, current_partner_id);
	let rep_select_options: Vec<CreateSelectMenuOption> = reps
		.iter()
		.map(|(rep_id, rep_name)| {
			CreateSelectMenuOption::new(rep_name.clone(), rep_id.to_string())
				.default_selection(rep_id.to_string() == *current_user_id)
		})
		.collect();

	let partner_select = CreateSelectMenu::new(
		partner_select_id,
		CreateSelectMenuKind::String {
			options: partner_select_options,
		},
	)
	.placeholder("Partner");
	let rep_select = CreateSelectMenu::new(
		rep_select_id,
		CreateSelectMenuKind::String {
			options: rep_select_options,
		},
	)
	.placeholder("Representative user");
	let submit_button = CreateButton::new(submit_button_id)
		.label("Submit")
		.style(ButtonStyle::Danger)
		.disabled(current_partner_id.is_empty() || current_user_id.is_empty());
	let cancel_button = CreateButton::new(cancel_button_id)
		.label("Cancel")
		.style(ButtonStyle::Secondary);

	let partner_row = CreateActionRow::SelectMenu(partner_select);
	let rep_row = CreateActionRow::SelectMenu(rep_select);
	let buttons_row = CreateActionRow::Buttons(vec![submit_button, cancel_button]);

	let mut components = vec![partner_row];
	if !reps.is_empty() {
		components.push(rep_row);
	}
	components.push(buttons_row);

	components
}

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let Some(guild) = command.guild_id else {
		bail!("Partners command used outside of a guild");
	};

	let sql_guild_id = guild.get() as i64;
	let db_connection = get_database_connection(ctx).await;
	let partners: Vec<Partner> = {
		let mut db_connection = db_connection.lock().await;
		if !guild_setup_check_with_reply(ctx, command, guild, &mut db_connection).await? {
			return Ok(());
		}

		partners::table
			.filter(partners::guild.eq(sql_guild_id))
			.order(partners::display_name.asc())
			.load(&mut *db_connection)
			.into_diagnostic()?
	};

	if partners.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("You have no partners for which to remove our representative.");
		command
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let mut current_partner_page = 0;

	let partner_select_id = cuid2::create_id();
	let rep_select_id = cuid2::create_id();
	let submit_button_id = cuid2::create_id();
	let cancel_button_id = cuid2::create_id();

	let components_data = ComponentsData {
		partners: &partners,
		reps: &Vec::new(),
		current_partner_page,
		current_partner_id: "",
		current_user_id: "",
		partner_select_id: &partner_select_id,
		rep_select_id: &rep_select_id,
		submit_button_id: &submit_button_id,
		cancel_button_id: &cancel_button_id,
	};
	let components = components_to_display(components_data);

	let message = CreateInteractionResponseMessage::new()
		.ephemeral(true)
		.content("Choose the partner server and the representative for that server to remove as our representative:")
		.components(components);
	command
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	let mut partnership_id = String::new();
	let mut user_id = String::new();
	let mut current_partner_reps = Vec::new();

	let interaction: ComponentInteraction = loop {
		let Some(interaction) = ComponentInteractionCollector::new(&ctx.shard)
			.custom_ids(vec![
				partner_select_id.clone(),
				rep_select_id.clone(),
				submit_button_id.clone(),
				cancel_button_id.clone(),
			])
			.timeout(Duration::from_secs(120))
			.await
		else {
			let message = EditInteractionResponse::new()
				.content("No representatives were removed.")
				.components(Vec::new());
			command.edit_response(&ctx.http, message).await.into_diagnostic()?;
			return Ok(());
		};

		match &interaction.data.kind {
			ComponentInteractionDataKind::StringSelect { values } => {
				let value = values.first().cloned().unwrap_or_default();
				if interaction.data.custom_id == partner_select_id {
					if value == "<" {
						partnership_id = String::new();
						current_partner_page = current_partner_page.saturating_sub(1);
					} else if value == ">" {
						partnership_id = String::new();
						current_partner_page = (current_partner_page + 1).min(max_partner_page(&partners));
					} else {
						partnership_id = value;
					}
					user_id = String::new();
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;

					let partner_rep_ids: Vec<i64> = if partnership_id.is_empty() {
						Vec::new()
					} else {
						let mut db_connection = db_connection.lock().await;
						partner_self_users::table
							.filter(partner_self_users::partnership.eq(&partnership_id))
							.select(partner_self_users::user_id)
							.load(&mut *db_connection)
							.into_diagnostic()?
					};
					let partner_rep_ids: Vec<u64> = partner_rep_ids.into_iter().map(|user_id| user_id as u64).collect();
					let mut partner_reps: Vec<(u64, String)> = Vec::with_capacity(partner_rep_ids.len());
					for rep_id in partner_rep_ids {
						let name = match guild.member(&ctx.http, rep_id).await {
							Ok(member) => match member.nick {
								Some(nick) => nick,
								None => match member.user.global_name {
									Some(name) => name,
									None => member.user.name,
								},
							},
							Err(SerenityError::Http(HttpError::UnsuccessfulRequest(ErrorResponse {
								status_code: StatusCode::NOT_FOUND,
								..
							}))) => match UserId::new(rep_id).to_user(&ctx.http).await {
								Ok(user) => match user.global_name {
									Some(name) => name,
									None => user.name,
								},
								Err(error) => bail!(error),
							},
							Err(error) => bail!(error),
						};
						partner_reps.push((rep_id, name));
					}

					current_partner_reps.clone_from(&partner_reps);

					let components_data = ComponentsData {
						partners: &partners,
						reps: &partner_reps,
						current_partner_page,
						current_partner_id: &partnership_id,
						current_user_id: &user_id,
						partner_select_id: &partner_select_id,
						rep_select_id: &rep_select_id,
						submit_button_id: &submit_button_id,
						cancel_button_id: &cancel_button_id,
					};
					let components = components_to_display(components_data);

					let new_message = EditInteractionResponse::new().components(components);
					command.edit_response(&ctx.http, new_message).await.into_diagnostic()?;
				} else if interaction.data.custom_id == rep_select_id {
					user_id = value;

					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
						.await
						.into_diagnostic()?;

					let components_data = ComponentsData {
						partners: &partners,
						reps: &current_partner_reps,
						current_partner_page,
						current_partner_id: &partnership_id,
						current_user_id: &user_id,
						partner_select_id: &partner_select_id,
						rep_select_id: &rep_select_id,
						submit_button_id: &submit_button_id,
						cancel_button_id: &cancel_button_id,
					};
					let components = components_to_display(components_data);

					let new_message = EditInteractionResponse::new().components(components);
					command.edit_response(&ctx.http, new_message).await.into_diagnostic()?;
				}
			}
			ComponentInteractionDataKind::Button => {
				if interaction.data.custom_id == submit_button_id {
					break interaction;
				}
				if interaction.data.custom_id == cancel_button_id {
					let message = CreateInteractionResponseMessage::new()
						.ephemeral(true)
						.content("Canceled partner representative removal.");
					interaction
						.create_response(&ctx.http, CreateInteractionResponse::Message(message))
						.await
						.into_diagnostic()?;
					return Ok(());
				}
			}
			_ => bail!(
				"Unexpected interaction type received for partners remove_rep command: {:?}",
				interaction.data.kind
			),
		}
	};

	if partnership_id.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No partner representative removed; no partner selected.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}
	if user_id.is_empty() {
		let message = CreateInteractionResponseMessage::new()
			.ephemeral(true)
			.content("No partner representative removed; no representative selected.");
		interaction
			.create_response(&ctx.http, CreateInteractionResponse::Message(message))
			.await
			.into_diagnostic()?;
		return Ok(());
	}

	let user_id: u64 = user_id.parse().into_diagnostic()?;
	let sql_user_id = user_id as i64;
	let mut db_connection = db_connection.lock().await;

	let partner_display_name = partners
		.iter()
		.find(|partner| partner.partnership_id == partnership_id)
		.map(|partner| partner.display_name.clone());
	let Some(partner_display_name) = partner_display_name else {
		bail!("Partner selections desynchronized with partner list");
	};

	diesel::delete(partner_self_users::table)
		.filter(
			partner_self_users::partnership
				.eq(&partnership_id)
				.and(partner_self_users::user_id.eq(sql_user_id)),
		)
		.execute(&mut *db_connection)
		.into_diagnostic()?;

	let message_content = format!(
		"Removed <@{}> as a representative for {}.",
		user_id, partner_display_name
	);
	let message = CreateInteractionResponseMessage::new()
		.content(message_content)
		.allowed_mentions(CreateAllowedMentions::new());
	interaction
		.create_response(&ctx.http, CreateInteractionResponse::Message(message))
		.await
		.into_diagnostic()?;

	Ok(())
}
