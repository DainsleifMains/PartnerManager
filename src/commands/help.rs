use crate::command_types::{CommandError, Context};
use miette::IntoDiagnostic;
use poise::builtins::HelpConfiguration;

/// Gets command help
#[poise::command(slash_command)]
pub async fn help(ctx: Context<'_>, command: Option<String>) -> Result<(), CommandError> {
	let config = HelpConfiguration {
		ephemeral: ctx.guild_id().is_some(),
		..Default::default()
	};
	poise::builtins::help(ctx, command.as_deref(), config)
		.await
		.into_diagnostic()
}
