use crate::command_types::{CommandError, CommandErrorValue, Context};

mod build_new;
use build_new::build_new;

mod edit_category;
use edit_category::edit_category;

#[poise::command(
	slash_command,
	guild_only,
	default_member_permissions = "MANAGE_GUILD",
	subcommands("build_new", "edit_category")
)]
pub async fn partner_embed(_ctx: Context<'_>) -> Result<(), CommandError> {
	Err(CommandErrorValue::BadParentCommand)?
}
