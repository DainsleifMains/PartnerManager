use crate::command_types::{CommandError, CommandErrorValue, Context};

mod add;
use add::add;

mod remove;
use remove::remove;

/// Manages partner servers
#[poise::command(
	slash_command,
	guild_only,
	default_member_permissions = "MANAGE_GUILD",
	subcommands("add", "remove")
)]
pub async fn partners(_ctx: Context<'_>) -> Result<(), CommandError> {
	Err(CommandErrorValue::BadParentCommand)?
}
