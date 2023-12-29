use crate::command_types::{CommandError, Data};
use poise::Command;

mod help;
use help::help;

mod partner_categories;
use partner_categories::partner_categories;

mod partner_embed;
use partner_embed::partner_embed;

mod partners;
use partners::partners;

mod settings;
use settings::settings;

mod setup;
use setup::setup;

pub fn get_all_commands() -> Vec<Command<Data, CommandError>> {
	vec![
		help(),
		partner_categories(),
		partner_embed(),
		partners(),
		setup(),
		settings(),
	]
}
