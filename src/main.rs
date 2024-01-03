use miette::IntoDiagnostic;
use serenity::client::Client;
use serenity::model::gateway::GatewayIntents;
use std::sync::Arc;
use tokio::sync::Mutex;

mod config;
use config::parse_config;

mod database;
use database::{connect_db, run_embedded_migrations, DatabaseConnection};

mod handler;
use handler::Handler;

mod commands;
mod models;
mod schema;
mod utils;

#[tokio::main]
async fn main() -> miette::Result<()> {
	let config = Arc::new(parse_config("config.kdl").await?);

	let mut db_connection = connect_db(&config)?;
	run_embedded_migrations(&mut db_connection)?;

	let db_connection = Arc::new(Mutex::new(db_connection));

	let intents = GatewayIntents::empty();

	let mut client = Client::builder(&config.discord_bot_token, intents)
		.event_handler(Handler)
		.await
		.into_diagnostic()?;
	{
		let mut data = client.data.write().await;
		data.insert::<DatabaseConnection>(db_connection);
	}

	client.start().await.into_diagnostic()?;

	Ok(())
}
