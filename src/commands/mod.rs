use crate::command_types::{CommandError, Data};
use poise::Command;

mod setup;
use setup::setup;

pub fn get_all_commands() -> Vec<Command<Data, CommandError>> {
	vec![setup()]
}
