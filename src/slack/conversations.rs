use serde::Deserialize;

use crate::error::{Error, Result};
use crate::search_query::{ResolvedChannel, ResolvedUser};
use crate::slack::client::SlackClient;

const CONVERSATION_TYPES: &str = "public_channel,private_channel,mpim,im";

pub fn resolve_channel(client: &SlackClient, input: &str) -> Result<ResolvedChannel> {
    if looks_like_channel_id(input) {
        return Ok(ResolvedChannel {
            id: input.to_owned(),
            name: input.to_owned(),
        });
    }

    let input_normalized = normalize_name(input);
    let mut matches = Vec::new();
    let mut cursor = String::new();

    loop {
        let mut query = vec![
            ("types", CONVERSATION_TYPES.to_owned()),
            ("exclude_archived", "true".to_owned()),
            ("limit", "200".to_owned()),
        ];
        if !cursor.is_empty() {
            query.push(("cursor", cursor.clone()));
        }

        let response: ConversationsListResponse = client.get("conversations.list", &query)?;
        for channel in response.channels {
            if normalize_name(&channel.name) == input_normalized {
                matches.push(ResolvedChannel {
                    id: channel.id,
                    name: channel.name,
                });
            }
        }

        cursor = response
            .response_metadata
            .and_then(|metadata| metadata.next_cursor)
            .unwrap_or_default();
        if cursor.is_empty() {
            break;
        }
    }

    match matches.len() {
        0 => Err(Error::ChannelNotFound(input.to_owned())),
        1 => Ok(matches.remove(0)),
        _ => Err(Error::AmbiguousChannel {
            name: input.to_owned(),
            matches: matches
                .into_iter()
                .map(|channel| format!("{} ({})", channel.name, channel.id))
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

pub fn resolve_user(client: &SlackClient, input: &str) -> Result<ResolvedUser> {
    if looks_like_user_id(input) {
        return Ok(ResolvedUser {
            id: input.to_owned(),
            name: input.to_owned(),
        });
    }

    let input_normalized = normalize_name(input);
    let mut matches = Vec::new();
    let mut cursor = String::new();

    loop {
        let mut query = vec![("limit", "200".to_owned())];
        if !cursor.is_empty() {
            query.push(("cursor", cursor.clone()));
        }

        let response: UsersListResponse = client.get("users.list", &query)?;
        for user in response.members {
            if user.deleted {
                continue;
            }
            if user
                .match_names()
                .iter()
                .any(|name| normalize_name(name) == input_normalized)
            {
                let name = user.display_name();
                matches.push(ResolvedUser { id: user.id, name });
            }
        }

        cursor = response
            .response_metadata
            .and_then(|metadata| metadata.next_cursor)
            .unwrap_or_default();
        if cursor.is_empty() {
            break;
        }
    }

    match matches.len() {
        0 => Err(Error::UserNotFound(input.to_owned())),
        1 => Ok(matches.remove(0)),
        _ => Err(Error::AmbiguousUser {
            name: input.to_owned(),
            matches: matches
                .into_iter()
                .map(|user| format!("{} ({})", user.name, user.id))
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

fn looks_like_channel_id(input: &str) -> bool {
    matches!(input.as_bytes().first(), Some(b'C' | b'G' | b'D'))
        && input.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn looks_like_user_id(input: &str) -> bool {
    matches!(input.as_bytes().first(), Some(b'U' | b'W'))
        && input.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn normalize_name(input: &str) -> String {
    input.trim().trim_start_matches('#').to_ascii_lowercase()
}

#[derive(Debug, Deserialize)]
struct ConversationsListResponse {
    channels: Vec<Conversation>,
    #[serde(default)]
    response_metadata: Option<ResponseMetadata>,
}

#[derive(Debug, Deserialize)]
struct Conversation {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct UsersListResponse {
    members: Vec<User>,
    #[serde(default)]
    response_metadata: Option<ResponseMetadata>,
}

#[derive(Debug, Deserialize)]
struct User {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    real_name: String,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    profile: UserProfile,
}

impl User {
    fn match_names(&self) -> Vec<&str> {
        [
            &self.name,
            &self.real_name,
            &self.profile.display_name,
            &self.profile.real_name,
        ]
        .into_iter()
        .map(String::as_str)
        .filter(|name| !name.is_empty())
        .collect()
    }

    fn display_name(&self) -> String {
        [&self.profile.display_name, &self.real_name, &self.name]
            .into_iter()
            .find(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| self.id.clone())
    }
}

#[derive(Debug, Default, Deserialize)]
struct UserProfile {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    real_name: String,
}

#[derive(Debug, Deserialize)]
struct ResponseMetadata {
    #[serde(default)]
    next_cursor: Option<String>,
}
