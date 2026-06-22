use std::io::{self, Write};
use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};

const AUTH_TOKEN_INSTRUCTIONS: &str = "\
Get a Slack Web API user token:
1. Open https://api.slack.com/apps and create a new app or select an existing app for the workspace you want to search.
2. Open OAuth & Permissions.
3. Under User Token Scopes, add these scopes:
   - search:read.public
   - search:read.private
   - search:read.im
   - search:read.mpim
   - search:read.users
   - search:read
   - channels:read
   - groups:read
   - im:read
   - mpim:read
   - users:read
4. Click Install to Workspace or Reinstall to Workspace and approve the scopes.
5. Back on OAuth & Permissions, copy the User OAuth Token. It should start with xoxp-.

Do not paste a Slack App Configuration Token; those are for app management and are not Web API access tokens.
";

pub fn capture_token(path: &Path) -> Result<()> {
    println!("{AUTH_TOKEN_INSTRUCTIONS}");
    print!("Slack Web API User OAuth Token: ");
    io::stdout().flush().map_err(Error::ReadToken)?;

    let token = read_token()?;
    let token = token.trim().to_owned();

    if token.is_empty() {
        return Err(Error::MissingToken);
    }

    save_token(path, token)
}

fn read_token() -> Result<String> {
    match rpassword::read_password() {
        Ok(token) => Ok(token),
        Err(terminal_error) => {
            let mut token = String::new();
            io::stdin()
                .read_line(&mut token)
                .map_err(|_| Error::ReadToken(terminal_error))?;
            Ok(token)
        }
    }
}

pub fn save_token(path: &Path, token: String) -> Result<()> {
    if path.exists() {
        Config::load_from_path(path)?;
    }
    let config = Config {
        slack_web_api_token: Some(token),
    };
    config.write_to_path(path, true)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{AUTH_TOKEN_INSTRUCTIONS, save_token};
    use crate::config::Config;

    #[test]
    fn auth_token_instructions_explain_how_to_create_and_copy_user_token() {
        for expected in [
            "https://api.slack.com/apps",
            "OAuth & Permissions",
            "User Token Scopes",
            "search:read.public",
            "search:read.private",
            "search:read.im",
            "search:read.mpim",
            "search:read.users",
            "channels:read",
            "groups:read",
            "im:read",
            "mpim:read",
            "users:read",
            "Install to Workspace",
            "User OAuth Token",
            "xoxp-",
            "Slack App Configuration Token",
        ] {
            assert!(AUTH_TOKEN_INSTRUCTIONS.contains(expected));
        }
    }

    #[test]
    fn save_token_rewrites_config_to_top_level_token_only() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("slacksearch.jsonc");
        fs::write(
            &path,
            r#"{
              "dry_run": false,
              "default_output": "markdown",
              "slack": {
                "api_base_url": "https://example.slack.com/api",
                "web_api_token": "old-token"
              },
              "search": {
                "api_strategy": "legacy",
                "page_size": 5,
                "max_results": 9,
                "rate_limit_retries": 3
              }
            }"#,
        )
        .unwrap();

        save_token(&path, "new-token".to_owned()).unwrap();

        let updated = Config::load_from_path(&path).unwrap();
        assert_eq!(updated.slack_web_api_token.as_deref(), Some("new-token"));
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("slack_web_api_token"));
        assert!(!contents.contains("dry_run"));
        assert!(!contents.contains("default_output"));
        assert!(!contents.contains("api_base_url"));
        assert!(!contents.contains("\"web_api_token\""));
        assert!(!contents.contains("api_strategy"));
        assert!(!contents.contains("page_size"));
        assert!(!contents.contains("max_results"));
        assert!(!contents.contains("rate_limit_retries"));
    }
}
