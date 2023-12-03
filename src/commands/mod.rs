use crate::command_types::{CommandError, Data};
use poise::Command;

mod settings;
use settings::settings;

mod setup;
use setup::setup;

pub fn get_all_commands() -> Vec<Command<Data, CommandError>> {
	vec![setup(), settings()]
}
