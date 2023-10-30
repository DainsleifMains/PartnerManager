use knuffel::Decode;
use miette::IntoDiagnostic;
use tokio::fs;

pub async fn parse_config(config_path: &str) -> miette::Result<ConfigDocument> {
	let config_file_contents = fs::read_to_string(config_path).await.into_diagnostic()?;
	let config = knuffel::parse(config_path, &config_file_contents)?;
	Ok(config)
}

#[derive(Debug, Decode)]
pub struct ConfigDocument {
	#[knuffel(child, unwrap(argument))]
	pub discord_bot_token: String,
	#[knuffel(child)]
	pub database: DatabaseArgs,
}

#[derive(Debug, Decode)]
pub struct DatabaseArgs {
	#[knuffel(child, unwrap(argument))]
	pub host: String,
	#[knuffel(child, unwrap(argument))]
	pub port: Option<u16>,
	#[knuffel(child, unwrap(argument))]
	pub username: String,
	#[knuffel(child, unwrap(argument))]
	pub password: String,
	#[knuffel(child, unwrap(argument))]
	pub database: String,
}
