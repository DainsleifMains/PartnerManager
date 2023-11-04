use diesel::prelude::*;
use miette::Diagnostic;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

pub struct Data {
	pub db_connection: Arc<Mutex<PgConnection>>,
}

pub type CommandError = miette::Report;
pub type Context<'a> = poise::Context<'a, Data, CommandError>;

#[derive(Debug, Diagnostic, Error)]
pub enum CommandErrorValue {
	#[error("expected guild in command data")]
	BadGuild,
	#[error("value was for the wrong guild")]
	WrongGuild,
}
