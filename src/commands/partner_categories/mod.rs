use miette::{bail, ensure, Severity};
use serenity::builder::{CreateCommand, CreateCommandOption};
use serenity::client::Context;
use serenity::model::application::{CommandInteraction, CommandOptionType, CommandType, ResolvedValue};
use serenity::model::permissions::Permissions;

mod add;
mod list;
mod remove;

pub fn definition() -> CreateCommand {
	let add_name_option =
		CreateCommandOption::new(CommandOptionType::String, "name", "The name to give the new category").required(true);
	let add_subcommand = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"add",
		"Adds a partnership category with the provided name",
	)
	.add_sub_option(add_name_option);

	let list_subcommand = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"list",
		"Lists partner categories that have been created for this server",
	);

	let remove_subcommand = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"remove",
		"Deletes a partnership category with the given name",
	);

	CreateCommand::new("partner_categories")
		.kind(CommandType::ChatInput)
		.default_member_permissions(Permissions::MANAGE_GUILD)
		.dm_permission(false)
		.description("Manages partner categories")
		.add_option(add_subcommand)
		.add_option(list_subcommand)
		.add_option(remove_subcommand)
}

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let options = command.data.options();
	ensure!(
		!options.is_empty(),
		severity = Severity::Error,
		"Insufficient subcommands passed to partner_categories command"
	);

	let subcommand = options.first().unwrap();
	let ResolvedValue::SubCommand(subcommand_options) = &subcommand.value else {
		bail!("Incorrect data type passed to partner_categories subcommand");
	};
	match subcommand.name {
		"add" => add::execute(ctx, command, subcommand_options).await,
		"list" => list::execute(ctx, command).await,
		"remove" => remove::execute(ctx, command).await,
		_ => bail!(
			"Invalid subcommand received for partner_categories command: {:?}",
			subcommand
		),
	}
}
