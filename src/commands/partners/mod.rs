use crate::command_types::{CommandError, CommandErrorValue, Context};

mod add;
use add::add;

mod add_rep;
use add_rep::add_rep;

mod remove;
use remove::remove;

mod remove_rep;
use remove_rep::remove_rep;

mod set_category;
use set_category::set_category;

mod set_name;
use set_name::set_name;

/// Manages partner servers
#[poise::command(
	slash_command,
	guild_only,
	default_member_permissions = "MANAGE_GUILD",
	subcommands("add", "add_rep", "remove", "remove_rep", "set_category", "set_name")
)]
pub async fn partners(_ctx: Context<'_>) -> Result<(), CommandError> {
	Err(CommandErrorValue::BadParentCommand)?
}
