use std::fs;
use std::path::{Path, PathBuf};

use jsonc_parser::parse_to_serde_value;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const EXAMPLE_CONFIG: &str = r#"{
  // Run `slacksearch auth` to store a real Slack Web API access token locally.
  "slack_web_api_token": null
}
"#;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub slack_web_api_token: Option<String>,
}

impl Config {
    pub fn default_path() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".slacksearch")
            .join("slacksearch.jsonc")
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(Error::MissingConfig(path.to_path_buf()));
        }

        let contents = fs::read_to_string(path).map_err(|source| Error::ReadConfig {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&contents, path)
    }

    pub fn parse(contents: &str, path: &Path) -> Result<Self> {
        let value = parse_to_serde_value(contents, &Default::default()).map_err(|error| {
            Error::ParseConfig {
                path: path.to_path_buf(),
                message: error.to_string(),
            }
        })?;

        let Some(value) = value else {
            return Ok(Self::default());
        };

        serde_json::from_value(value).map_err(|error| Error::ParseConfig {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
    }

    pub fn validate(&self) -> Result<()> {
        let token = self
            .slack_web_api_token
            .as_deref()
            .unwrap_or_default()
            .trim();
        if token.is_empty() || token == "xoxp-your-token-here" || token == "xoxb-your-token-here" {
            return Err(Error::MissingToken);
        }

        Ok(())
    }

    pub fn write_example(path: &Path, force: bool) -> Result<()> {
        write_config(path, EXAMPLE_CONFIG, force)
    }

    pub fn write_to_path(&self, path: &Path, force: bool) -> Result<()> {
        let json =
            serde_json::to_string_pretty(self).expect("config serialization should not fail");
        let contents = format!("{json}\n");
        write_config(path, &contents, force)
    }
    #[cfg(test)]
    pub fn with_token(token: String) -> Self {
        Self {
            slack_web_api_token: Some(token),
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

fn write_config(path: &Path, contents: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(Error::ConfigExists(path.to_path_buf()));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::WriteConfig {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(path, contents).map_err(|source| Error::WriteConfig {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Config, EXAMPLE_CONFIG};
    use crate::error::Error;

    #[test]
    fn parses_jsonc_example_config() {
        let config = Config::parse(EXAMPLE_CONFIG, Path::new("example")).unwrap();

        assert!(config.slack_web_api_token.is_none());
    }

    #[test]
    fn default_example_config_requires_real_token() {
        let config = Config::parse(EXAMPLE_CONFIG, Path::new("example")).unwrap();

        let error = config.validate().unwrap_err();

        assert!(matches!(error, Error::MissingToken));
    }

    #[test]
    fn rejects_invalid_jsonc() {
        let error = Config::parse("{ nope", Path::new("broken.jsonc")).unwrap_err();

        assert!(matches!(error, Error::ParseConfig { .. }));
    }

    #[test]
    fn default_path_uses_slacksearch_config_under_home() {
        let path = Config::default_path();

        assert!(path.ends_with(".slacksearch/slacksearch.jsonc"));
        if let Some(home) = dirs::home_dir() {
            assert!(path.starts_with(home));
        }
    }

    #[test]
    fn validates_real_token() {
        let config = Config::with_token("xoxp-real-token".to_owned());

        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_missing_null_and_placeholder_tokens() {
        for contents in [
            r#"{}"#,
            r#"{"slack_web_api_token": null}"#,
            r#"{"slack_web_api_token": ""}"#,
            r#"{"slack_web_api_token": "   "}"#,
            r#"{"slack_web_api_token": "xoxp-your-token-here"}"#,
            r#"{"slack_web_api_token": "xoxb-your-token-here"}"#,
        ] {
            let config = Config::parse(contents, Path::new("missing.jsonc")).unwrap();

            let error = config.validate().unwrap_err();

            assert!(matches!(error, Error::MissingToken));
        }
    }

    #[test]
    fn ignores_obsolete_fields_but_does_not_read_legacy_nested_token() {
        let config = Config::parse(
            r#"{
              "dry_run": false,
              "default_output": "xml",
              "slack": {"web_api_token": "xoxp-real-token"}
            }"#,
            Path::new("obsolete.jsonc"),
        )
        .unwrap();

        let error = config.validate().unwrap_err();
        assert!(matches!(error, Error::MissingToken));
        let serialized = serde_json::to_value(&config).unwrap();
        assert_eq!(serialized.get("default_output"), None);
        assert_eq!(serialized.get("dry_run"), None);
        assert_eq!(serialized.get("slack"), None);
    }

    #[test]
    fn make_config_refuses_to_overwrite_without_force() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("slacksearch.jsonc");

        Config::write_example(&path, false).unwrap();
        let error = Config::write_example(&path, false).unwrap_err();

        assert!(matches!(error, Error::ConfigExists(existing) if existing == path));
    }
}
