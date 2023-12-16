use crate::command_types::{CommandError, Data};
use poise::Command;

mod partner_categories;
use partner_categories::partner_categories;

mod settings;
use settings::settings;

mod setup;
use setup::setup;

pub fn get_all_commands() -> Vec<Command<Data, CommandError>> {
	vec![partner_categories(), setup(), settings()]
}
