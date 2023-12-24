use crate::command_types::{CommandError, CommandErrorValue, Context};

mod add;
use add::add;

mod list;
use list::list;

mod remove;
use remove::remove;

/// Manage categories of partners
#[poise::command(
	slash_command,
	guild_only,
	default_member_permissions = "MANAGE_GUILD",
	subcommands("add", "list", "remove")
)]
pub async fn partner_categories(_ctx: Context<'_>) -> Result<(), CommandError> {
	Err(CommandErrorValue::BadParentCommand)?
}
