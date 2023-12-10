use crate::command_types::{CommandError, CommandErrorValue, Context};

pub mod embed_channel;
use embed_channel::embed_channel;

/// Set or view settings for the server
///
/// Allows setting all settings (required or optional) for the server.
#[poise::command(
	slash_command,
	guild_only,
	default_member_permissions = "MANAGE_GUILD",
	subcommands("embed_channel")
)]
pub async fn settings(_ctx: Context<'_>) -> Result<(), CommandError> {
	Err(CommandErrorValue::BadParentCommand)?
}
