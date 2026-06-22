use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::output::{SearchMessage, SearchResult, SearchResults};
use crate::search_query::{ResolvedSearchRequest, SearchRequest};
use crate::slack::client::SlackClient;
use crate::slack::conversations::{resolve_channel, resolve_user};

pub fn search(client: &SlackClient, request: SearchRequest) -> Result<SearchResults> {
    let resolved = resolve(client, request)?;

    match search_assistant(client, &resolved) {
        Err(Error::SlackApi { method, error, .. })
            if method == "assistant.search.context"
                && matches!(
                    error.as_str(),
                    "feature_not_enabled"
                        | "assistant_search_context_disabled"
                        | "deprecated_endpoint"
                        | "method_not_supported_for_channel_type"
                        | "missing_scope"
                ) =>
        {
            search_legacy(client, &resolved)
        }
        other => other,
    }
}

fn resolve(client: &SlackClient, request: SearchRequest) -> Result<ResolvedSearchRequest> {
    let channel = request
        .search_channel
        .as_deref()
        .map(|channel| resolve_channel(client, channel))
        .transpose()?;
    let user = request
        .search_chat
        .as_deref()
        .map(|user| resolve_user(client, user))
        .transpose()?;

    Ok(ResolvedSearchRequest {
        query: request.query,
        before: request.before,
        after: request.after,
        channel,
        user,
        page_size: request.page_size,
        max_results: request.max_results,
        context_msg_cnt: request.context_msg_cnt,
    })
}

fn search_assistant(
    client: &SlackClient,
    request: &ResolvedSearchRequest,
) -> Result<SearchResults> {
    let mut cursor = None;
    let mut results = Vec::new();
    let limit = request.page_size.min(20);

    while results.len() < request.max_results {
        let payload = AssistantSearchRequest {
            query: request.assistant_query(),
            content_types: vec!["messages"],
            channel_types: vec!["public_channel", "private_channel", "mpim", "im"],
            limit,
            cursor: cursor.clone(),
            before: request.before.as_ref().map(|date| date.unix),
            after: request.after.as_ref().map(|date| date.unix),
            highlight: true,
            include_context_messages: request.context_msg_cnt > 0,
        };
        let response: AssistantSearchResponse =
            client.post_json("assistant.search.context", &payload)?;

        if let Some(search_results) = response.results {
            results.extend(
                search_results
                    .messages
                    .into_iter()
                    .map(|message| message.into_search_result(request.context_msg_cnt)),
            );
        }
        results.truncate(request.max_results);

        cursor = response
            .response_metadata
            .and_then(|metadata| metadata.next_cursor)
            .filter(|cursor| !cursor.is_empty());
        if cursor.is_none() {
            break;
        }
    }

    fill_missing_user_names(client, &mut results)?;

    Ok(SearchResults {
        query: request.query.clone(),
        results,
    })
}

fn search_legacy(client: &SlackClient, request: &ResolvedSearchRequest) -> Result<SearchResults> {
    let mut cursor = Some("*".to_owned());
    let mut results = Vec::new();
    let count = request.page_size.min(100);

    while results.len() < request.max_results {
        let mut query = vec![
            ("query", request.legacy_query()),
            ("count", count.to_string()),
            ("highlight", "true".to_owned()),
            ("sort", "timestamp".to_owned()),
            ("sort_dir", "desc".to_owned()),
        ];
        if let Some(cursor_value) = cursor.clone() {
            query.push(("cursor", cursor_value));
        }

        let response: LegacySearchResponse = client.get("search.messages", &query)?;
        results.extend(
            response
                .messages
                .matches
                .into_iter()
                .map(LegacyMatch::into_search_result),
        );
        results.truncate(request.max_results);

        cursor = response
            .messages
            .pagination
            .and_then(|pagination| pagination.next_cursor)
            .filter(|cursor| !cursor.is_empty());
        if cursor.is_none() {
            break;
        }
    }

    fill_missing_user_names(client, &mut results)?;

    Ok(SearchResults {
        query: request.query.clone(),
        results,
    })
}

#[derive(Debug, Serialize)]
struct AssistantSearchRequest<'a> {
    query: String,
    content_types: Vec<&'a str>,
    channel_types: Vec<&'a str>,
    limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<i64>,
    highlight: bool,
    include_context_messages: bool,
}

#[derive(Debug, Deserialize)]
struct AssistantSearchResponse {
    #[serde(default)]
    results: Option<AssistantResults>,
    #[serde(default)]
    response_metadata: Option<ResponseMetadata>,
}

#[derive(Debug, Deserialize)]
struct AssistantResults {
    #[serde(default)]
    messages: Vec<AssistantMessage>,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    #[serde(default)]
    channel_id: Option<String>,
    #[serde(default)]
    channel_name: Option<String>,
    #[serde(default)]
    author_user_id: Option<String>,
    #[serde(default)]
    author_name: Option<String>,
    #[serde(default)]
    message_ts: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    permalink: Option<String>,
    #[serde(default)]
    context_messages: AssistantContextMessages,
}

#[derive(Debug, Default, Deserialize)]
struct AssistantContextMessages {
    #[serde(default)]
    before: Vec<AssistantContextMessage>,
    #[serde(default)]
    after: Vec<AssistantContextMessage>,
}

#[derive(Debug, Deserialize)]
struct AssistantContextMessage {
    #[serde(default)]
    channel_id: Option<String>,
    #[serde(default)]
    channel_name: Option<String>,
    #[serde(default, alias = "user_id:")]
    user_id: Option<String>,
    #[serde(default)]
    author_name: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    ts: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    permalink: Option<String>,
}

impl AssistantMessage {
    fn into_search_result(self, context_msg_cnt: usize) -> SearchResult {
        let channel_id = blank_to_none(self.channel_id);
        let channel_name = blank_to_none(self.channel_name);
        let permalink = blank_to_none(self.permalink);
        let permalink_base = permalink.as_deref().and_then(parent_permalink_base);
        let before = self
            .context_messages
            .before
            .into_iter()
            .take(context_msg_cnt)
            .map(|message| {
                message.into_search_message(
                    channel_id.clone(),
                    channel_name.clone(),
                    permalink_base.as_deref(),
                )
            })
            .collect();
        let after = self
            .context_messages
            .after
            .into_iter()
            .take(context_msg_cnt)
            .map(|message| {
                message.into_search_message(
                    channel_id.clone(),
                    channel_name.clone(),
                    permalink_base.as_deref(),
                )
            })
            .collect();

        SearchResult {
            channel_id,
            channel_name,
            user_id: blank_to_none(self.author_user_id),
            user_name: blank_to_none(self.author_name),
            timestamp: self.message_ts,
            text: self.content,
            permalink,
            before,
            after,
        }
    }
}

impl AssistantContextMessage {
    fn into_search_message(
        self,
        parent_channel_id: Option<String>,
        parent_channel_name: Option<String>,
        parent_permalink_base: Option<&str>,
    ) -> SearchMessage {
        let channel_id = blank_to_none(self.channel_id).or(parent_channel_id);
        let timestamp = self.ts;
        let permalink = blank_to_none(self.permalink).or_else(|| {
            derive_context_permalink(parent_permalink_base, channel_id.as_deref(), &timestamp)
        });

        SearchMessage {
            channel_id,
            channel_name: blank_to_none(self.channel_name).or(parent_channel_name),
            user_id: blank_to_none(self.user_id),
            user_name: blank_to_none(self.author_name).or_else(|| blank_to_none(self.username)),
            timestamp,
            text: self.text,
            permalink,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LegacySearchResponse {
    messages: LegacyMessages,
}

#[derive(Debug, Deserialize)]
struct LegacyMessages {
    #[serde(default)]
    matches: Vec<LegacyMatch>,
    #[serde(default)]
    pagination: Option<ResponseMetadata>,
}

#[derive(Debug, Deserialize)]
struct LegacyMatch {
    #[serde(default)]
    channel: Option<LegacyChannel>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    ts: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    permalink: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LegacyChannel {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

impl LegacyMatch {
    fn into_search_result(self) -> SearchResult {
        let channel_id = self
            .channel
            .as_ref()
            .and_then(|channel| blank_to_none(channel.id.clone()));
        let channel_name = self.channel.and_then(|channel| blank_to_none(channel.name));

        SearchResult {
            channel_id,
            channel_name,
            user_id: blank_to_none(self.user),
            user_name: blank_to_none(self.username),
            timestamp: self.ts,
            text: self.text,
            permalink: blank_to_none(self.permalink),
            before: Vec::new(),
            after: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ResponseMetadata {
    #[serde(default)]
    next_cursor: Option<String>,
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_owned())
        }
    })
}

fn parent_permalink_base(permalink: &str) -> Option<String> {
    let (base, _) = permalink.split_once("/archives/")?;
    if base.trim().is_empty() {
        None
    } else {
        Some(format!("{base}/archives"))
    }
}

fn derive_context_permalink(
    parent_base: Option<&str>,
    channel_id: Option<&str>,
    timestamp: &str,
) -> Option<String> {
    let parent_base = parent_base?;
    let channel_id = channel_id?.trim();
    let timestamp = timestamp.trim().replace('.', "");

    if channel_id.is_empty() || timestamp.is_empty() {
        None
    } else {
        Some(format!("{parent_base}/{channel_id}/p{timestamp}"))
    }
}

fn fill_missing_user_names(client: &SlackClient, results: &mut [SearchResult]) -> Result<()> {
    let user_ids = missing_user_name_ids(results);
    if user_ids.is_empty() {
        return Ok(());
    }

    let users = fetch_user_display_names(client, &user_ids)?;
    for result in results {
        fill_message_user_name(result.as_message_mut(), &users);
        for message in &mut result.before {
            fill_message_user_name(message.as_message_mut(), &users);
        }
        for message in &mut result.after {
            fill_message_user_name(message.as_message_mut(), &users);
        }
    }

    Ok(())
}

fn missing_user_name_ids(results: &[SearchResult]) -> HashSet<String> {
    let mut user_ids = HashSet::new();
    for result in results {
        collect_missing_user_name_id(result.as_message_ref(), &mut user_ids);
        for message in &result.before {
            collect_missing_user_name_id(message.as_message_ref(), &mut user_ids);
        }
        for message in &result.after {
            collect_missing_user_name_id(message.as_message_ref(), &mut user_ids);
        }
    }
    user_ids
}

fn collect_missing_user_name_id(message: SearchMessageRef<'_>, user_ids: &mut HashSet<String>) {
    if message.user_name.is_none_or(|name| name.trim().is_empty())
        && let Some(user_id) = message.user_id
    {
        user_ids.insert(user_id.to_owned());
    }
}

fn fill_message_user_name(message: SearchMessageMut<'_>, users: &HashMap<String, String>) {
    if message.user_name.as_deref().is_none_or(str::is_empty)
        && let Some(user_id) = message.user_id.as_ref().map(String::as_str)
        && let Some(user_name) = users.get(user_id)
    {
        *message.user_name = Some(user_name.clone());
    }
}

fn fetch_user_display_names(
    client: &SlackClient,
    needed_user_ids: &HashSet<String>,
) -> Result<HashMap<String, String>> {
    let mut users = HashMap::new();
    let mut cursor = String::new();

    loop {
        let mut query = vec![("limit", "200".to_owned())];
        if !cursor.is_empty() {
            query.push(("cursor", cursor.clone()));
        }

        let response: UsersListResponse = client.get("users.list", &query)?;
        for user in response.members {
            if !user.deleted && needed_user_ids.contains(&user.id) {
                users.insert(user.id.clone(), user.display_name());
            }
        }

        if users.len() == needed_user_ids.len() {
            break;
        }

        cursor = response
            .response_metadata
            .and_then(|metadata| metadata.next_cursor)
            .unwrap_or_default();
        if cursor.is_empty() {
            break;
        }
    }

    Ok(users)
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
    fn display_name(&self) -> String {
        [
            &self.profile.display_name,
            &self.profile.real_name,
            &self.real_name,
            &self.name,
        ]
        .into_iter()
        .find(|name| !name.trim().is_empty())
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

struct SearchMessageRef<'a> {
    user_id: Option<&'a str>,
    user_name: Option<&'a String>,
}

struct SearchMessageMut<'a> {
    user_id: &'a Option<String>,
    user_name: &'a mut Option<String>,
}

impl SearchResult {
    fn as_message_ref(&self) -> SearchMessageRef<'_> {
        SearchMessageRef {
            user_id: self.user_id.as_deref(),
            user_name: self.user_name.as_ref(),
        }
    }

    fn as_message_mut(&mut self) -> SearchMessageMut<'_> {
        SearchMessageMut {
            user_id: &self.user_id,
            user_name: &mut self.user_name,
        }
    }
}

impl SearchMessage {
    fn as_message_ref(&self) -> SearchMessageRef<'_> {
        SearchMessageRef {
            user_id: self.user_id.as_deref(),
            user_name: self.user_name.as_ref(),
        }
    }

    fn as_message_mut(&mut self) -> SearchMessageMut<'_> {
        SearchMessageMut {
            user_id: &self.user_id,
            user_name: &mut self.user_name,
        }
    }
}
