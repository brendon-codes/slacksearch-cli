use serde_json::Value;

pub struct MockHttpResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

impl MockHttpResponse {
    fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: body.into(),
        }
    }
}

pub fn handle(method: &str, target: &str, body: &str) -> MockHttpResponse {
    let path = target.split('?').next().unwrap_or(target);

    match (method, path) {
        ("GET", "/health") => MockHttpResponse::json(200, r#"{"ok":true}"#),
        ("GET", "/conversations.list") => MockHttpResponse::json(200, conversations_list()),
        ("GET", "/users.list") => MockHttpResponse::json(200, users_list()),
        ("GET", "/search.messages") => MockHttpResponse::json(200, legacy_search()),
        ("POST", "/assistant.search.context") => {
            MockHttpResponse::json(200, assistant_search(body))
        }
        _ => MockHttpResponse::json(
            404,
            format!(
                r#"{{"ok":false,"error":"unknown_mock_endpoint","method":{method:?},"path":{path:?}}}"#
            ),
        ),
    }
}

fn assistant_search(body: &str) -> String {
    let payload = serde_json::from_str::<Value>(body).unwrap_or(Value::Null);
    let query = payload
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let cursor = payload
        .get("cursor")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if query.contains("force legacy") {
        return r#"{"ok":false,"error":"feature_not_enabled"}"#.to_owned();
    }

    if query.contains("missing") {
        return r#"{"ok":true,"results":{"messages":[]},"response_metadata":{"next_cursor":""}}"#
            .to_owned();
    }

    if cursor == "page-2" {
        return r#"{
  "ok": true,
  "results": {
    "messages": [
      {
        "author_name": "Grace Hopper",
        "author_user_id": "U456",
        "team_id": "T123",
        "channel_id": "C456",
        "channel_name": "engineering",
        "message_ts": "1508284297.000015",
        "content": "The roadmap follow-up is scheduled.",
        "permalink": "https://example.slack.com/archives/C456/p1508284297000015"
      }
    ]
  },
  "response_metadata": {"next_cursor": ""}
}"#
        .to_owned();
    }

    r#"{
  "ok": true,
  "results": {
    "messages": [
      {
        "author_name": "Ada Lovelace",
        "author_user_id": "U123",
        "team_id": "T123",
        "channel_id": "C123",
        "channel_name": "general",
        "message_ts": "1508284197.000015",
        "content": "The roadmap is ready.",
        "permalink": "https://example.slack.com/archives/C123/p1508284197000015"
      }
    ]
  },
  "response_metadata": {"next_cursor": "page-2"}
}"#
    .to_owned()
}

fn conversations_list() -> &'static str {
    r#"{
  "ok": true,
  "channels": [
    {"id": "C123", "name": "general"},
    {"id": "C456", "name": "engineering"},
    {"id": "C789", "name": "general-discussion"}
  ],
  "response_metadata": {"next_cursor": ""}
}"#
}

fn users_list() -> &'static str {
    r#"{
  "ok": true,
  "members": [
    {
      "id": "U123",
      "name": "ada",
      "real_name": "Ada Lovelace",
      "profile": {"display_name": "Ada Lovelace", "real_name": "Ada Lovelace"}
    },
    {
      "id": "U456",
      "name": "john",
      "real_name": "John Doe",
      "profile": {"display_name": "John Doe", "real_name": "John Doe"}
    }
  ],
  "response_metadata": {"next_cursor": ""}
}"#
}

fn legacy_search() -> &'static str {
    r#"{
  "ok": true,
  "messages": {
    "matches": [
      {
        "channel": {"id": "C123", "name": "general"},
        "user": "U123",
        "username": "Ada Lovelace",
        "ts": "1508284197.000015",
        "text": "The legacy roadmap result is ready.",
        "permalink": "https://example.slack.com/archives/C123/p1508284197000015"
      }
    ],
    "pagination": {"next_cursor": ""}
  }
}"#
}

#[cfg(test)]
mod tests {
    use super::handle;

    #[test]
    fn returns_assistant_search_fixture() {
        let response = handle(
            "POST",
            "/assistant.search.context",
            r#"{"query":"roadmap"}"#,
        );

        assert_eq!(response.status, 200);
        assert!(response.body.contains("Ada Lovelace"));
        assert!(response.body.contains("page-2"));
    }

    #[test]
    fn returns_legacy_fallback_error_for_selected_query() {
        let response = handle(
            "POST",
            "/assistant.search.context",
            r#"{"query":"force legacy"}"#,
        );

        assert_eq!(response.status, 200);
        assert!(response.body.contains("feature_not_enabled"));
    }
}
