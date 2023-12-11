use miette::IntoDiagnostic;
use poise::{Framework, FrameworkOptions};
use serenity::model::gateway::GatewayIntents;
use std::sync::Arc;
use tokio::sync::Mutex;

mod commands;
use commands::get_all_commands;

mod command_types;
use command_types::Data;

mod config;
use config::parse_config;

mod database;
use database::{connect_db, run_embedded_migrations};

mod models;
mod schema;
mod standard_replies;

#[tokio::main]
async fn main() -> miette::Result<()> {
	let config = Arc::new(parse_config("config.kdl").await?);

	let mut db_connection = connect_db(&config)?;
	run_embedded_migrations(&mut db_connection)?;

	let db_connection = Arc::new(Mutex::new(db_connection));

	let commands = get_all_commands();

	let framework = Framework::builder()
		.options(FrameworkOptions {
			commands,
			..Default::default()
		})
		.token(&config.discord_bot_token)
		.intents(GatewayIntents::GUILD_INTEGRATIONS)
		.setup(|ctx, _ready, framework| {
			Box::pin(async move {
				poise::builtins::register_globally(ctx, &framework.options().commands)
					.await
					.into_diagnostic()?;
				Ok(Data { db_connection })
			})
		});

	framework.run().await.into_diagnostic()
}
