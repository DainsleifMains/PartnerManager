use serenity::async_trait;
use serenity::model::application::{Command, Interaction};
use serenity::model::gateway::Ready;
use serenity::prelude::*;

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
	async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
		if let Interaction::Command(command) = interaction {
			let command_result = match command.data.name.as_str() {
				"partner_categories" => crate::commands::partner_categories::execute(&ctx, &command).await,
				"partner_embed" => crate::commands::partner_embed::execute(&ctx, &command).await,
				"partners" => crate::commands::partners::execute(&ctx, &command).await,
				"settings" => crate::commands::settings::execute(&ctx, &command).await,
				"setup" => crate::commands::setup::execute(&ctx, &command).await,
				_ => unimplemented!(),
			};

			if let Err(error) = command_result {
				eprintln!("Command error: {}", error);
			}
		}
	}

	async fn ready(&self, ctx: Context, _data_about_bot: Ready) {
		let commands = vec![
			crate::commands::partner_categories::definition(),
			crate::commands::partner_embed::definition(),
			crate::commands::partners::definition(),
			crate::commands::settings::definition(),
			crate::commands::setup::definition(),
		];
		Command::set_global_commands(&ctx.http, commands)
			.await
			.expect("Failed to register commands");
	}
}
