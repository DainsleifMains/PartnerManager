use miette::{bail, ensure, Severity};
use serenity::builder::{CreateCommand, CreateCommandOption};
use serenity::client::Context;
use serenity::model::application::{CommandInteraction, CommandOptionType, CommandType};
use serenity::model::permissions::Permissions;

mod build_new;
mod edit_category;
mod edit_content;

pub fn definition() -> CreateCommand {
	let build_new_subcommand = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"build_new",
		"Opens a builder form to create a new embed",
	);
	let edit_category_subcommand = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"edit_category",
		"Edits the partner category for an embed",
	);
	let edit_content_subcommand = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"edit_content",
		"Edits the content of an embed",
	);

	CreateCommand::new("partner_embed")
		.kind(CommandType::ChatInput)
		.default_member_permissions(Permissions::MANAGE_GUILD)
		.dm_permission(false)
		.description("Manage the embed listing partners")
		.add_option(build_new_subcommand)
		.add_option(edit_category_subcommand)
		.add_option(edit_content_subcommand)
}

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let options = command.data.options();
	ensure!(
		!options.is_empty(),
		severity = Severity::Error,
		"Subcommands not passed to the partner_embed command"
	);
	let subcommand = &options[0];
	match subcommand.name {
		"build_new" => build_new::execute(ctx, command).await,
		"edit_category" => edit_category::execute(ctx, command).await,
		"edit_content" => edit_content::execute(ctx, command).await,
		_ => bail!(
			"Unexpected subcommand passed to the partner_embed command: {:?}",
			subcommand
		),
	}
}
