use diesel::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Data {
	pub db_connection: Arc<Mutex<PgConnection>>,
}

pub type CommandError = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, CommandError>;
