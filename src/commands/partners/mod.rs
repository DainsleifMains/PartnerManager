use miette::{bail, ensure, Severity};
use serenity::builder::{CreateCommand, CreateCommandOption};
use serenity::client::Context;
use serenity::model::application::{CommandInteraction, CommandOptionType, CommandType, ResolvedValue};
use serenity::model::permissions::Permissions;

mod add;
mod add_rep;
mod add_self_rep;
mod list_reps;
mod list_self_reps;
mod remove;
mod remove_rep;
mod remove_self_rep;
mod set_category;
mod set_name;
mod user_rep_list;

pub fn definition() -> CreateCommand {
	let partner_add_invite_link = CreateCommandOption::new(
		CommandOptionType::String,
		"invite_link",
		"A permanent invite link for the guild",
	)
	.required(true);
	let partner_add_display_name = CreateCommandOption::new(
		CommandOptionType::String,
		"display_name",
		"Display name for the server; defaults to the server name",
	)
	.required(false);
	let add_partner_command = CreateCommandOption::new(CommandOptionType::SubCommand, "add", "Adds a partner server")
		.add_sub_option(partner_add_invite_link)
		.add_sub_option(partner_add_display_name);

	let add_representative_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"add_rep",
		"Adds a representative for a particular partner",
	);
	let list_representatives_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"list_reps",
		"Lists representatives for a particular partner",
	);
	let remove_partner_command =
		CreateCommandOption::new(CommandOptionType::SubCommand, "remove", "Removes a partner server");
	let remove_representative_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"remove_rep",
		"Removes a representative for a particular partner",
	);
	let set_category_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"set_category",
		"Sets the partner category for an existing partner",
	);

	let new_name =
		CreateCommandOption::new(CommandOptionType::String, "new_display_name", "The new name to use").required(true);
	let set_name_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"set_name",
		"Sets the display name for a partner",
	)
	.add_sub_option(new_name);

	let add_self_representative_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"add_self_rep",
		"Adds our representative for a server",
	);
	let list_self_representative_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"list_self_reps",
		"Lists our representatives to the partner",
	);
	let remove_self_representative_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"remove_self_rep",
		"Remove one of our representatives for a partner",
	);

	let partner_rep_list_user = CreateCommandOption::new(
		CommandOptionType::User,
		"user",
		"The user for which to list servers represented",
	)
	.required(true);
	let user_rep_list = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"user_rep_list",
		"Lists servers represented by a user and for which that user represents us",
	)
	.add_sub_option(partner_rep_list_user);

	CreateCommand::new("partners")
		.kind(CommandType::ChatInput)
		.default_member_permissions(Permissions::MANAGE_GUILD)
		.dm_permission(false)
		.description("Manages partners and their representatives for the server")
		.add_option(add_partner_command)
		.add_option(add_representative_command)
		.add_option(list_representatives_command)
		.add_option(remove_partner_command)
		.add_option(remove_representative_command)
		.add_option(set_category_command)
		.add_option(set_name_command)
		.add_option(add_self_representative_command)
		.add_option(list_self_representative_command)
		.add_option(remove_self_representative_command)
		.add_option(user_rep_list)
}

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let options = command.data.options();
	ensure!(
		!options.is_empty(),
		severity = Severity::Error,
		"called the partners command without subcommands"
	);
	let subcommand = options.first().unwrap();
	let ResolvedValue::SubCommand(subcommand_options) = &subcommand.value else {
		bail!("Incorrect data type passed for partners subcommand option");
	};
	match subcommand.name {
		"add" => add::execute(ctx, command, subcommand_options).await,
		"add_rep" => add_rep::execute(ctx, command).await,
		"add_self_rep" => add_self_rep::execute(ctx, command).await,
		"list_reps" => list_reps::execute(ctx, command).await,
		"list_self_reps" => list_self_reps::execute(ctx, command).await,
		"remove" => remove::execute(ctx, command).await,
		"remove_rep" => remove_rep::execute(ctx, command).await,
		"remove_self_rep" => remove_self_rep::execute(ctx, command).await,
		"set_category" => set_category::execute(ctx, command).await,
		"set_name" => set_name::execute(ctx, command, subcommand_options).await,
		"user_rep_list" => user_rep_list::execute(ctx, command, subcommand_options).await,
		_ => bail!("Unexpected subcommand for partners command: {:?}", subcommand),
	}
}
