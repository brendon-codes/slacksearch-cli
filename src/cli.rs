use std::fmt;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "slacksearch")]
#[command(about = "Search Slack messages from the terminal")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Search(SearchCommand),
    MakeConfig(MakeConfigCommand),
    ValidateConfig(ValidateConfigCommand),
    Auth(AuthCommand),
    Server(ServerCommand),
}

#[derive(Debug, Args)]
pub struct SearchCommand {
    #[arg(long = "config")]
    pub config_path: Option<PathBuf>,
    #[arg(long = "api-base-url")]
    pub api_base_url: Option<String>,
    #[arg(long)]
    pub before: Option<String>,
    #[arg(long)]
    pub after: Option<String>,
    #[arg(long = "search-channel")]
    pub search_channel: Option<String>,
    #[arg(long = "search-chat")]
    pub search_chat: Option<String>,
    #[arg(long, conflicts_with_all = ["text", "markdown"])]
    pub json: bool,
    #[arg(long, conflicts_with_all = ["json", "markdown"])]
    pub text: bool,
    #[arg(long, conflicts_with_all = ["json", "text"])]
    pub markdown: bool,
    #[arg(long)]
    pub max_results: Option<usize>,
    #[arg(long)]
    pub page_size: Option<usize>,
    #[arg(long = "context-msg-cnt")]
    pub context_msg_cnt: Option<usize>,
    pub query: String,
}

#[derive(Debug, Args)]
pub struct MakeConfigCommand {
    #[arg(long = "config")]
    pub config_path: Option<PathBuf>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ValidateConfigCommand {
    #[arg(long = "config")]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuthCommand {
    #[arg(long = "config")]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ServerCommand {
    #[arg(long, default_value = "127.0.0.1:3000")]
    pub bind: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
    Markdown,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => formatter.write_str("text"),
            Self::Json => formatter.write_str("json"),
            Self::Markdown => formatter.write_str("markdown"),
        }
    }
}

impl SearchCommand {
    pub fn output(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else if self.markdown {
            OutputFormat::Markdown
        } else if self.text {
            OutputFormat::Text
        } else {
            OutputFormat::Json
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command, OutputFormat};

    #[test]
    fn parses_search_examples() {
        let cli = Cli::parse_from([
            "slacksearch",
            "search",
            "--api-base-url",
            "http://127.0.0.1:3000",
            "--before",
            "2026-05-01 12:31:01-06",
            "--after",
            "2026-03-01",
            "--search-channel",
            "general-discussion",
            "--context-msg-cnt",
            "2",
            "--json",
            "find this phrase",
        ]);

        let Command::Search(command) = cli.command else {
            panic!("expected search command");
        };

        assert_eq!(command.before.as_deref(), Some("2026-05-01 12:31:01-06"));
        assert_eq!(
            command.api_base_url.as_deref(),
            Some("http://127.0.0.1:3000")
        );
        assert_eq!(command.after.as_deref(), Some("2026-03-01"));
        assert_eq!(
            command.search_channel.as_deref(),
            Some("general-discussion")
        );
        assert_eq!(command.context_msg_cnt, Some(2));
        assert_eq!(command.query, "find this phrase");
        assert_eq!(command.output(), OutputFormat::Json);
    }

    #[test]
    fn search_output_defaults_to_json_when_no_flag_is_present() {
        let cli = Cli::parse_from(["slacksearch", "search", "find this phrase"]);

        let Command::Search(command) = cli.command else {
            panic!("expected search command");
        };

        assert_eq!(command.output(), OutputFormat::Json);
    }
}
