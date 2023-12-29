use miette::IntoDiagnostic;
use poise::builtins::on_error as default_on_error;
use poise::framework::Framework;
use poise::structs::{FrameworkError, FrameworkOptions};
use serenity::client::ClientBuilder;
use serenity::model::gateway::GatewayIntents;
use std::sync::Arc;
use tokio::sync::Mutex;

mod commands;
use commands::get_all_commands;

mod command_types;
use command_types::{CommandError, Data};

mod config;
use config::parse_config;

mod database;
use database::{connect_db, run_embedded_migrations};

mod models;
mod schema;
mod utils;

async fn on_error(error: FrameworkError<'_, Data, CommandError>) {
	match error {
		FrameworkError::Setup { error, .. } => panic!("A setup error occurred: {}", error),
		FrameworkError::EventHandler { error, .. } => eprintln!("Event error: {}", error),
		FrameworkError::Command { error, .. } => eprintln!("Command error: {}", error),
		FrameworkError::CommandPanic { payload, .. } => eprintln!("Command panic: {:?}", payload),
		FrameworkError::ArgumentParse { error, input, .. } => {
			eprintln!("Failed to parse argument ({:?}): {}", input, error)
		}
		FrameworkError::CooldownHit { .. } => default_on_error(error).await.unwrap(),
		FrameworkError::CommandCheckFailed { .. } => default_on_error(error).await.unwrap(),
		_ => panic!("Unexpected error: {}", error),
	}
}

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
			on_error: |error| Box::pin(on_error(error)),
			..Default::default()
		})
		.setup(|ctx, _ready, framework| {
			Box::pin(async move {
				poise::builtins::register_globally(ctx, &framework.options().commands)
					.await
					.into_diagnostic()?;
				Ok(Data { db_connection })
			})
		})
		.build();

	let client = ClientBuilder::new(&config.discord_bot_token, GatewayIntents::GUILD_INTEGRATIONS)
		.framework(framework)
		.await;
	client.into_diagnostic()?.start().await.into_diagnostic()
}
