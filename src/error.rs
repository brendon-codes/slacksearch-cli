use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config file does not exist at {0}")]
    MissingConfig(PathBuf),
    #[error("config file already exists at {0}; pass --force to overwrite it")]
    ConfigExists(PathBuf),
    #[error("failed to read {path}: {source}")]
    ReadConfig {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write {path}: {source}")]
    WriteConfig {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse JSONC in {path}: {message}")]
    ParseConfig { path: PathBuf, message: String },
    #[error(
        "missing Slack Web API token; run `slacksearch auth` or set slack_web_api_token in the config"
    )]
    MissingToken,
    #[error("failed to read token from terminal: {0}")]
    ReadToken(std::io::Error),
    #[error("invalid {field} value {value:?}: {message}")]
    InvalidDate {
        field: &'static str,
        value: String,
        message: String,
    },
    #[error("failed to call Slack {method}: {message}")]
    SlackHttp {
        method: &'static str,
        message: String,
    },
    #[error("Slack {method} returned HTTP {status}: {body}")]
    SlackHttpStatus {
        method: &'static str,
        status: u16,
        body: String,
    },
    #[error("Slack {method} returned error {error}{details}")]
    SlackApi {
        method: &'static str,
        error: String,
        details: String,
    },
    #[error("Slack {method} returned a malformed response: {message}")]
    MalformedSlackResponse {
        method: &'static str,
        message: String,
    },
    #[error("failed to bind mock Slack API server at {bind}: {source}")]
    MockServerBind {
        bind: String,
        source: std::io::Error,
    },
    #[error("mock Slack API server I/O failed: {0}")]
    MockServerIo(std::io::Error),
    #[error("channel {0:?} was not found in visible Slack conversations")]
    ChannelNotFound(String),
    #[error("channel {name:?} matched multiple visible conversations: {matches}")]
    AmbiguousChannel { name: String, matches: String },
    #[error("chat/person {0:?} was not found in the visible Slack user list")]
    UserNotFound(String),
    #[error("chat/person {name:?} matched multiple Slack users: {matches}")]
    AmbiguousUser { name: String, matches: String },
    #[error("--page-size must be between 1 and 100")]
    InvalidPageSize,
    #[error("--max-results must be greater than 0")]
    InvalidMaxResults,
    #[error("--context-msg-cnt must be between 0 and 20")]
    InvalidContextMsgCnt,
}
