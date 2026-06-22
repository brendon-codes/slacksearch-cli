mod auth;
mod cli;
mod config;
mod error;
mod mock_slack;
mod output;
mod search_query;
mod server;
mod slack;
mod time;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::config::Config;
use crate::error::Result;
use crate::search_query::SearchRequest;
use crate::slack::client::SlackClient;

const DEFAULT_SLACK_API_BASE_URL: &str = "https://slack.com/api";
const DEFAULT_RATE_LIMIT_RETRIES: u32 = 1;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Search(command) => {
            let path = command
                .config_path
                .clone()
                .unwrap_or_else(Config::default_path);
            let config = Config::load_from_path(&path)?;
            config.validate()?;
            let token = config
                .slack_web_api_token
                .clone()
                .ok_or(crate::error::Error::MissingToken)?;
            let output_format = command.output();
            let request = SearchRequest::from_cli(&command)?;
            let client = SlackClient::new(
                token,
                command
                    .api_base_url
                    .clone()
                    .unwrap_or_else(|| DEFAULT_SLACK_API_BASE_URL.to_owned()),
                DEFAULT_RATE_LIMIT_RETRIES,
            );
            let results = slack::search::search(&client?, request)?;
            print!("{}", output::render(&results, output_format)?);
            Ok(())
        }
        Command::MakeConfig(command) => {
            let path = command.config_path.unwrap_or_else(Config::default_path);
            Config::write_example(&path, command.force)?;
            println!("created {}", path.display());
            Ok(())
        }
        Command::ValidateConfig(command) => {
            let path = command.config_path.unwrap_or_else(Config::default_path);
            let config = Config::load_from_path(&path)?;
            config.validate()?;
            println!("valid {}", path.display());
            Ok(())
        }
        Command::Auth(command) => {
            let path = command.config_path.unwrap_or_else(Config::default_path);
            auth::capture_token(&path)?;
            println!("saved Slack Web API token in {}", path.display());
            Ok(())
        }
        Command::Server(command) => server::run(&command.bind),
    }
}
