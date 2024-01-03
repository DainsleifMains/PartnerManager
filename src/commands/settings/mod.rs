use miette::{bail, ensure, Severity};
use serenity::builder::{CreateCommand, CreateCommandOption};
use serenity::client::Context;
use serenity::model::application::{CommandInteraction, CommandOptionType, CommandType, ResolvedValue};
use serenity::model::channel::ChannelType;
use serenity::model::permissions::Permissions;

mod embed_channel;
mod partner_role;

pub fn definition() -> CreateCommand {
	let get_embed_channel_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"get",
		"Get the channel to which the embed is published",
	);
	let set_embed_channel_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"set",
		"Change the channel to which the embed is published",
	)
	.add_sub_option(
		CreateCommandOption::new(
			CommandOptionType::Channel,
			"embed_channel",
			"The channel in which to show the partnership embed",
		)
		.required(true)
		.channel_types(vec![ChannelType::Text]),
	);
	let embed_channel_command = CreateCommandOption::new(
		CommandOptionType::SubCommandGroup,
		"embed_channel",
		"The channel to which the embed is published",
	)
	.add_sub_option(get_embed_channel_command)
	.add_sub_option(set_embed_channel_command);

	let get_partner_role_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"get",
		"Gets the role assigned to all partner representatives",
	);
	let set_partner_role_command = CreateCommandOption::new(
		CommandOptionType::SubCommand,
		"set",
		"Sets the role assigned to all partner reprsesntatives",
	)
	.add_sub_option(
		CreateCommandOption::new(
			CommandOptionType::Role,
			"partner_role",
			"The role to assign to all partner representatives; leave blank to clear",
		)
		.required(false),
	);
	let partner_role_command = CreateCommandOption::new(
		CommandOptionType::SubCommandGroup,
		"partner_role",
		"The role to assign to all partner representatives",
	)
	.add_sub_option(get_partner_role_command)
	.add_sub_option(set_partner_role_command);

	CreateCommand::new("settings")
		.kind(CommandType::ChatInput)
		.default_member_permissions(Permissions::MANAGE_GUILD)
		.dm_permission(false)
		.description("Manages settings for partner management for the server")
		.add_option(embed_channel_command)
		.add_option(partner_role_command)
}

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> miette::Result<()> {
	let options = command.data.options();
	ensure!(
		!options.is_empty(),
		severity = Severity::Error,
		"not enough subcommands passed to settings command"
	);
	let subcommand = options.first().unwrap();
	let ResolvedValue::SubCommandGroup(subcommand_options) = &subcommand.value else {
		bail!("Incorrect data type for settings subcommands: {:?}", subcommand);
	};
	match subcommand.name {
		"embed_channel" => embed_channel::execute(ctx, command, subcommand_options).await,
		"partner_role" => partner_role::execute(ctx, command, subcommand_options).await,
		_ => bail!("Unexpected subcommand for settings: {}", subcommand.name),
	}
}
