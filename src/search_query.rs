use crate::cli::SearchCommand;
use crate::error::{Error, Result};
use crate::time::{DateFilter, parse_date_filter};

const DEFAULT_PAGE_SIZE: usize = 20;
const DEFAULT_MAX_RESULTS: usize = 100;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SearchRequest {
    pub query: String,
    pub before: Option<DateFilter>,
    pub after: Option<DateFilter>,
    pub search_channel: Option<String>,
    pub search_chat: Option<String>,
    pub page_size: usize,
    pub max_results: usize,
    pub context_msg_cnt: usize,
}

impl SearchRequest {
    pub fn from_cli(command: &SearchCommand) -> Result<Self> {
        let page_size = command.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        if !(1..=100).contains(&page_size) {
            return Err(Error::InvalidPageSize);
        }

        let max_results = command.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        if max_results == 0 {
            return Err(Error::InvalidMaxResults);
        }

        let context_msg_cnt = command.context_msg_cnt.unwrap_or(0);
        if context_msg_cnt > 20 {
            return Err(Error::InvalidContextMsgCnt);
        }

        Ok(Self {
            query: command.query.clone(),
            before: command
                .before
                .as_deref()
                .map(|value| parse_date_filter("before", value))
                .transpose()?,
            after: command
                .after
                .as_deref()
                .map(|value| parse_date_filter("after", value))
                .transpose()?,
            search_channel: command.search_channel.clone(),
            search_chat: command.search_chat.clone(),
            page_size,
            max_results,
            context_msg_cnt,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvedSearchRequest {
    pub query: String,
    pub before: Option<DateFilter>,
    pub after: Option<DateFilter>,
    pub channel: Option<ResolvedChannel>,
    pub user: Option<ResolvedUser>,
    pub page_size: usize,
    pub max_results: usize,
    pub context_msg_cnt: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvedChannel {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvedUser {
    pub id: String,
    pub name: String,
}

impl ResolvedSearchRequest {
    pub fn assistant_query(&self) -> String {
        let mut parts = vec![self.query.clone()];
        if let Some(channel) = &self.channel {
            parts.push(format!("in:<#{}>", channel.id));
        }
        if let Some(user) = &self.user {
            parts.push(format!("with:<@{}>", user.id));
        }
        parts.join(" ")
    }

    pub fn legacy_query(&self) -> String {
        let mut parts = vec![self.query.clone()];
        if let Some(before) = &self.before {
            parts.push(format!("before:{}", before.date));
        }
        if let Some(after) = &self.after {
            parts.push(format!("after:{}", after.date));
        }
        if let Some(channel) = &self.channel {
            parts.push(format!("in:<#{}>", channel.id));
        }
        if let Some(user) = &self.user {
            parts.push(format!("in:<@{}>", user.id));
        }
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::{Cli, Command};
    use crate::search_query::{
        ResolvedChannel, ResolvedSearchRequest, ResolvedUser, SearchRequest,
    };

    #[test]
    fn builds_search_request_from_cli_filters() {
        let cli = Cli::parse_from([
            "slacksearch",
            "search",
            "--before",
            "2026-05-01",
            "--after",
            "2026-03-01",
            "--search-channel",
            "general",
            "--search-chat",
            "John Doe",
            "--context-msg-cnt",
            "3",
            "--max-results",
            "25",
            "roadmap",
        ]);

        let Command::Search(command) = cli.command else {
            panic!("expected search command");
        };
        let request = SearchRequest::from_cli(&command).unwrap();

        assert_eq!(request.query, "roadmap");
        assert_eq!(request.before.unwrap().date, "2026-05-01");
        assert_eq!(request.after.unwrap().date, "2026-03-01");
        assert_eq!(request.search_channel.as_deref(), Some("general"));
        assert_eq!(request.search_chat.as_deref(), Some("John Doe"));
        assert_eq!(request.max_results, 25);
        assert_eq!(request.context_msg_cnt, 3);
    }

    #[test]
    fn rejects_oversized_context_count() {
        let cli = Cli::parse_from([
            "slacksearch",
            "search",
            "--context-msg-cnt",
            "21",
            "roadmap",
        ]);

        let Command::Search(command) = cli.command else {
            panic!("expected search command");
        };
        let error = SearchRequest::from_cli(&command).unwrap_err();

        assert!(matches!(error, crate::error::Error::InvalidContextMsgCnt));
    }

    #[test]
    fn defaults_search_limits() {
        let cli = Cli::parse_from(["slacksearch", "search", "roadmap"]);

        let Command::Search(command) = cli.command else {
            panic!("expected search command");
        };
        let request = SearchRequest::from_cli(&command).unwrap();

        assert_eq!(request.page_size, 20);
        assert_eq!(request.max_results, 100);
    }

    #[test]
    fn resolved_query_adds_api_specific_modifiers() {
        let request = ResolvedSearchRequest {
            query: "roadmap".to_owned(),
            before: None,
            after: None,
            channel: Some(ResolvedChannel {
                id: "C123".to_owned(),
                name: "general".to_owned(),
            }),
            user: Some(ResolvedUser {
                id: "U123".to_owned(),
                name: "John Doe".to_owned(),
            }),
            page_size: 20,
            max_results: 100,
            context_msg_cnt: 0,
        };

        assert_eq!(request.assistant_query(), "roadmap in:<#C123> with:<@U123>");
        assert_eq!(request.legacy_query(), "roadmap in:<#C123> in:<@U123>");
    }
}
