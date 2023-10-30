use miette::IntoDiagnostic;
use poise::{Framework, FrameworkOptions};
use serenity::model::gateway::GatewayIntents;
use std::sync::Arc;
use tokio::sync::Mutex;

mod command_types;
use command_types::{CommandError, Context, Data};

mod config;
use config::parse_config;

mod database;
use database::{connect_db, run_embedded_migrations};

/// Responds with a pong
#[poise::command(slash_command)]
async fn ping(ctx: Context<'_>) -> Result<(), CommandError> {
	ctx.say("Pong!").await?;
	Ok(())
}

#[tokio::main]
async fn main() -> miette::Result<()> {
	let config = Arc::new(parse_config("config.kdl").await?);

	let mut db_connection = connect_db(&config)?;
	run_embedded_migrations(&mut db_connection)?;

	let db_connection = Arc::new(Mutex::new(db_connection));

	let framework = Framework::builder()
		.options(FrameworkOptions {
			commands: vec![ping()],
			..Default::default()
		})
		.token(&config.discord_bot_token)
		.intents(GatewayIntents::GUILD_INTEGRATIONS)
		.setup(|ctx, _ready, framework| {
			Box::pin(async move {
				poise::builtins::register_globally(ctx, &framework.options().commands).await?;
				Ok(Data { db_connection })
			})
		});

	framework.run().await.into_diagnostic()
}
