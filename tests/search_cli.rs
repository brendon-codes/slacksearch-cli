use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[test]
fn search_cli_assistant_success_text_output() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        include_str!("fixtures/assistant_search_success.json"),
    )]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--text",
        "roadmap",
    ]);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("channel_name: general"));
    assert!(stdout.contains("user_name: Ada Lovelace"));
    assert!(stdout.contains("text: The roadmap is ready."));
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/assistant.search.context");
    assert!(requests[0].body.contains("\"query\":\"roadmap\""));
    assert!(requests[0].body.contains("\"channel_types\""));
}

#[test]
fn search_cli_paginates_until_cursor_is_empty() {
    let server = MockServer::start(vec![
        MockResponse::json(
            200,
            r#"{
              "ok": true,
              "results": {"messages": [{
                "author_name": "Ada",
                "author_user_id": "U123",
                "channel_id": "C123",
                "channel_name": "general",
                "message_ts": "1508284197.000015",
                "content": "first",
                "permalink": "https://example.slack.com/1"
              }]},
              "response_metadata": {"next_cursor": "NEXT"}
            }"#,
        ),
        MockResponse::json(
            200,
            r#"{
              "ok": true,
              "results": {"messages": [{
                "author_name": "Grace",
                "author_user_id": "U456",
                "channel_id": "C123",
                "channel_name": "general",
                "message_ts": "1508284297.000015",
                "content": "second",
                "permalink": "https://example.slack.com/2"
              }]},
              "response_metadata": {"next_cursor": ""}
            }"#,
        ),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--page-size",
        "1",
        "--max-results",
        "10",
        "--text",
        "roadmap",
    ]);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("user_name: Ada"));
    assert!(stdout.contains("text: first"));
    assert!(stdout.contains("user_name: Grace"));
    assert!(stdout.contains("text: second"));
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].body.contains("\"cursor\":\"NEXT\""));
}

#[test]
fn search_cli_resolves_channel_name_before_scoping_search() {
    let server = MockServer::start(vec![
        MockResponse::json(
            200,
            include_str!("fixtures/conversations_list_success.json"),
        ),
        MockResponse::json(200, include_str!("fixtures/assistant_search_success.json")),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--search-channel",
        "general",
        "roadmap",
    ]);

    assert_success(&output);
    let requests = server.finish();
    assert!(requests[0].path.starts_with("/conversations.list?"));
    assert!(requests[1].body.contains("in:<#C123>"));
}

#[test]
fn search_cli_resolves_chat_name_before_scoping_search() {
    let server = MockServer::start(vec![
        MockResponse::json(200, include_str!("fixtures/users_list_success.json")),
        MockResponse::json(200, include_str!("fixtures/assistant_search_success.json")),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--search-chat",
        "John Doe",
        "roadmap",
    ]);

    assert_success(&output);
    let requests = server.finish();
    assert!(requests[0].path.starts_with("/users.list?"));
    assert!(requests[1].body.contains("with:<@U123>"));
}

#[test]
fn search_cli_sends_date_filters_as_unix_timestamps() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        include_str!("fixtures/assistant_search_success.json"),
    )]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--after",
        "2026-03-01",
        "--before",
        "2026-05-01",
        "roadmap",
    ]);

    assert_success(&output);
    let requests = server.finish();
    assert!(requests[0].body.contains("\"before\":1777593600"));
    assert!(requests[0].body.contains("\"after\":1772323200"));
}

#[test]
fn search_cli_rejects_ambiguous_chat_name() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{
          "ok": true,
          "members": [
            {"id": "U123", "name": "john1", "real_name": "John Doe", "profile": {"display_name": "John Doe"}},
            {"id": "U456", "name": "john2", "real_name": "John Doe", "profile": {"display_name": "John Doe"}}
          ],
          "response_metadata": {"next_cursor": ""}
        }"#,
    )]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--search-chat",
        "John Doe",
        "roadmap",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("matched multiple Slack users"));
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
}

#[test]
fn search_cli_reports_http_errors() {
    let server = MockServer::start(vec![MockResponse::new(500, "server failed")]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--text",
        "roadmap",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Slack assistant.search.context returned HTTP 500"));
    server.finish();
}

#[test]
fn search_cli_reports_malformed_slack_responses() {
    let server = MockServer::start(vec![MockResponse::new(200, "not json")]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--text",
        "roadmap",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("returned a malformed response"));
    server.finish();
}

#[test]
fn search_cli_retries_rate_limited_search_response() {
    let server = MockServer::start(vec![
        MockResponse::new(429, "rate limited").with_header("Retry-After", "0"),
        MockResponse::json(200, include_str!("fixtures/assistant_search_success.json")),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "roadmap",
    ]);

    assert_success(&output);
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
}

#[test]
fn search_cli_prints_no_results() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{
          "ok": true,
          "results": {"messages": []},
          "response_metadata": {"next_cursor": ""}
        }"#,
    )]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--text",
        "missing",
    ]);

    assert_success(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "No results found.\n"
    );
    server.finish();
}

#[test]
fn search_cli_reports_slack_api_errors() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{"ok": false, "error": "invalid_auth"}"#,
    )]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--text",
        "roadmap",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Slack assistant.search.context returned error invalid_auth"));
    server.finish();
}

#[test]
fn search_cli_reports_slack_scope_details() {
    let server = MockServer::start(vec![
        MockResponse::json(200, r#"{"ok": false, "error": "missing_scope"}"#),
        MockResponse::json(
            200,
            r#"{"ok": false, "error": "missing_scope", "needed": "search:read", "provided": "channels:read"}"#,
        ),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "roadmap",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(
        "Slack search.messages returned error missing_scope (needed: search:read; provided: channels:read)"
    ));
    server.finish();
}

#[test]
fn search_cli_explains_app_configuration_token_scope_errors() {
    let server = MockServer::start(vec![
        MockResponse::json(200, r#"{"ok": false, "error": "missing_scope"}"#),
        MockResponse::json(
            200,
            r#"{"ok": false, "error": "missing_scope", "needed": "search:read", "provided": "identify,app_configurations:read,app_configurations:write"}"#,
        ),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "roadmap",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(
        "Slack search.messages returned error missing_scope (needed: search:read; provided: identify,app_configurations:read,app_configurations:write"
    ));
    assert!(stderr.contains("this token appears to be a Slack App Configuration Token"));
    assert!(stderr.contains("use a Slack Web API user token"));
    server.finish();
}

#[test]
fn search_cli_falls_back_to_legacy_search() {
    let server = MockServer::start(vec![
        MockResponse::json(200, r#"{"ok": false, "error": "missing_scope"}"#),
        MockResponse::json(200, include_str!("fixtures/legacy_search_success.json")),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--text",
        "roadmap",
    ]);

    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("text: legacy result"));
    let requests = server.finish();
    assert_eq!(requests[0].path, "/assistant.search.context");
    assert!(requests[1].path.starts_with("/search.messages?"));
}

#[test]
fn search_cli_renders_json_and_markdown_outputs() {
    let server = MockServer::start(vec![
        MockResponse::json(200, include_str!("fixtures/assistant_search_success.json")),
        MockResponse::json(200, include_str!("fixtures/assistant_search_success.json")),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let json = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--json",
        "roadmap",
    ]);
    let markdown = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--markdown",
        "roadmap",
    ]);

    assert_success(&json);
    assert_success(&markdown);
    assert!(String::from_utf8_lossy(&json.stdout).contains("\"channel_name\": \"general\""));
    assert!(String::from_utf8_lossy(&markdown.stdout).contains("| user_name | Ada Lovelace |"));
    server.finish();
}

#[test]
fn search_cli_defaults_to_json_when_config_omits_default_output() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        include_str!("fixtures/assistant_search_success.json"),
    )]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "roadmap",
    ]);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"query\": \"roadmap\""));
    assert!(stdout.contains("\"results\": ["));
    assert!(stdout.contains("\"channel_name\": \"general\""));
    server.finish();
}

#[test]
fn search_cli_text_flag_forces_console_output() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        include_str!("fixtures/assistant_search_success.json"),
    )]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--text",
        "roadmap",
    ]);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("query: roadmap"));
    assert!(stdout.contains("user_name: Ada Lovelace"));
    assert!(!stdout.contains("\"results\": ["));
    server.finish();
}

#[test]
fn search_cli_renders_assistant_context_messages() {
    let server = MockServer::start(vec![
        MockResponse::json(
            200,
            r#"{
              "ok": true,
              "results": {"messages": [{
                "author_name": "Ada Lovelace",
                "author_user_id": "U123",
                "channel_id": "C123",
                "channel_name": "general",
                "message_ts": "1508284197.000015",
                "content": "The roadmap is ready.",
                "permalink": "https://example.slack.com/archives/C123/p1508284197000015",
                "context_messages": {
                  "before": [
                    {"user_id": "U456", "author_name": "Grace Hopper", "ts": "1508284097.000015", "text": "Earlier context"},
                    {"user_id": "U789", "author_name": "Ignored", "ts": "1508284090.000015", "text": "Ignored context"}
                  ],
                  "after": [
                    {"channel_id": "C999", "user_id": "U123", "author_name": "Ada Lovelace", "ts": "1508284297.000015", "text": "Later context"}
                  ]
                }
              }]},
              "response_metadata": {"next_cursor": ""}
            }"#,
        ),
        MockResponse::json(
            200,
            r#"{
              "ok": true,
              "results": {"messages": [{
                "author_name": "Ada Lovelace",
                "author_user_id": "U123",
                "channel_id": "C123",
                "channel_name": "general",
                "message_ts": "1508284197.000015",
                "content": "The roadmap is ready.",
                "context_messages": {
                  "before": [{"user_id": "U456", "author_name": "Grace Hopper", "ts": "1508284097.000015", "text": "Earlier context"}],
                  "after": [{"user_id": "U123", "author_name": "Ada Lovelace", "ts": "1508284297.000015", "text": "Later context"}]
                }
              }]},
              "response_metadata": {"next_cursor": ""}
            }"#,
        ),
        MockResponse::json(
            200,
            r#"{
              "ok": true,
              "results": {"messages": [{
                "author_name": "Ada Lovelace",
                "author_user_id": "U123",
                "channel_id": "C123",
                "channel_name": "general",
                "message_ts": "1508284197.000015",
                "content": "The roadmap is ready.",
                "context_messages": {
                  "before": [{"user_id": "U456", "author_name": "Grace Hopper", "ts": "1508284097.000015", "text": "Earlier context"}],
                  "after": [{"user_id": "U123", "author_name": "Ada Lovelace", "ts": "1508284297.000015", "text": "Later context"}]
                }
              }]},
              "response_metadata": {"next_cursor": ""}
            }"#,
        ),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let json = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--context-msg-cnt",
        "1",
        "--json",
        "roadmap",
    ]);
    let text = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--context-msg-cnt",
        "1",
        "--text",
        "roadmap",
    ]);
    let markdown = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--context-msg-cnt",
        "1",
        "--markdown",
        "roadmap",
    ]);

    assert_success(&json);
    assert_success(&text);
    assert_success(&markdown);
    let json_stdout = String::from_utf8_lossy(&json.stdout);
    assert!(json_stdout.contains("\"before\": ["));
    assert!(json_stdout.contains("\"after\": ["));
    assert!(json_stdout.contains("\"text\": \"Earlier context\""));
    assert!(!json_stdout.contains("Ignored context"));
    let json_value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(
        json_value["results"][0]["before"][0]["permalink"],
        "https://example.slack.com/archives/C123/p1508284097000015"
    );
    assert_eq!(
        json_value["results"][0]["after"][0]["permalink"],
        "https://example.slack.com/archives/C999/p1508284297000015"
    );
    let text_stdout = String::from_utf8_lossy(&text.stdout);
    assert!(text_stdout.contains("before:"));
    assert!(text_stdout.contains("user_name: Grace Hopper"));
    assert!(text_stdout.contains("after:"));
    assert!(text_stdout.contains("text: Later context"));
    let markdown_stdout = String::from_utf8_lossy(&markdown.stdout);
    assert!(markdown_stdout.contains("### Before"));
    assert!(markdown_stdout.contains("| user_name | Grace Hopper |"));
    assert!(markdown_stdout.contains("### After"));
    assert!(markdown_stdout.contains("| text | Later context |"));
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert!(
        requests[0]
            .body
            .contains("\"include_context_messages\":true")
    );
}

#[test]
fn search_cli_leaves_context_permalink_null_when_it_cannot_be_derived() {
    let server = MockServer::start(vec![MockResponse::json(
        200,
        r#"{
          "ok": true,
          "results": {"messages": [{
            "author_name": "Ada Lovelace",
            "author_user_id": "U123",
            "channel_name": "general",
            "message_ts": "1508284197.000015",
            "content": "The roadmap is ready.",
            "context_messages": {
              "before": [{"user_id": "U456", "author_name": "Grace Hopper", "ts": "1508284097.000015", "text": "Earlier context"}],
              "after": []
            }
          }]},
          "response_metadata": {"next_cursor": ""}
        }"#,
    )]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--context-msg-cnt",
        "1",
        "--json",
        "roadmap",
    ]);

    assert_success(&output);
    let json_value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json_value["results"][0]["before"][0]["permalink"].is_null());
    server.finish();
}

#[test]
fn search_cli_fills_empty_author_names_from_users_list() {
    let server = MockServer::start(vec![
        MockResponse::json(
            200,
            r#"{
              "ok": true,
              "results": {"messages": [{
                "author_name": "",
                "author_user_id": "U123",
                "channel_id": "C123",
                "channel_name": "general",
                "message_ts": "1508284197.000015",
                "content": "The roadmap is ready.",
                "context_messages": {
                  "before": [{"user_id": "U456", "author_name": "", "ts": "1508284097.000015", "text": "Earlier context"}],
                  "after": []
                }
              }]},
              "response_metadata": {"next_cursor": ""}
            }"#,
        ),
        MockResponse::json(
            200,
            r#"{
              "ok": true,
              "members": [
                {"id": "U123", "name": "ada", "real_name": "Ada Lovelace", "profile": {"display_name": ""}},
                {"id": "U456", "name": "grace", "real_name": "", "profile": {"display_name": "Grace Hopper"}}
              ],
              "response_metadata": {"next_cursor": ""}
            }"#,
        ),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--context-msg-cnt",
        "1",
        "--json",
        "roadmap",
    ]);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"user_name\": \"Ada Lovelace\""));
    assert!(stdout.contains("\"user_name\": \"Grace Hopper\""));
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].path.split('?').next().unwrap(), "/users.list");
}

#[test]
fn search_cli_keeps_legacy_bot_username_when_user_id_is_empty() {
    let server = MockServer::start(vec![
        MockResponse::json(200, r#"{"ok": false, "error": "missing_scope"}"#),
        MockResponse::json(
            200,
            r#"{
          "ok": true,
          "messages": {
            "matches": [{
              "channel": {"id": "C123", "name": "general"},
              "user": "",
              "username": "robot overlord",
              "ts": "1508795665.000236",
              "text": "bot result",
              "permalink": "https://example.slack.com/bot"
            }],
            "pagination": {"next_cursor": ""}
          }
        }"#,
        ),
    ]);
    let tempdir = tempfile::tempdir().unwrap();
    let api_base_url = server.url();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &api_base_url,
        "--json",
        "bot",
    ]);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"user_id\": null"));
    assert!(stdout.contains("\"user_name\": \"robot overlord\""));
    assert_eq!(server.finish().len(), 2);
}

fn run_slacksearch<const N: usize>(args: [&str; N]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_slacksearch"))
        .args(args)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    if !output.status.success() {
        panic!(
            "expected command to succeed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn write_config(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("slacksearch.jsonc");
    fs::write(
        &path,
        r#"{
          "slack_web_api_token": "xoxp-test-token"
        }"#,
    )
    .unwrap();
    path
}

#[derive(Debug, Clone)]
struct RecordedRequest {
    path: String,
    body: String,
}

struct MockResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl MockResponse {
    fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_owned(),
        }
    }

    fn json(status: u16, body: &str) -> Self {
        Self::new(status, body).with_header("Content-Type", "application/json")
    }

    fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

struct MockServer {
    addr: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    handle: JoinHandle<()>,
}

impl MockServer {
    fn start(responses: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let thread_responses = Arc::clone(&responses);
        let thread_requests = Arc::clone(&requests);

        let handle = thread::spawn(move || {
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(15) {
                if thread_responses.lock().unwrap().is_empty() {
                    break;
                }

                match listener.accept() {
                    Ok((stream, _)) => {
                        handle_connection(stream, &thread_responses, &thread_requests)
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("mock server accept failed: {error}"),
                }
            }
        });

        Self {
            addr,
            requests,
            handle,
        }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn finish(self) -> Vec<RecordedRequest> {
        self.handle.join().unwrap();
        self.requests.lock().unwrap().clone()
    }
}

fn handle_connection(
    mut stream: std::net::TcpStream,
    responses: &Arc<Mutex<VecDeque<MockResponse>>>,
    requests: &Arc<Mutex<Vec<RecordedRequest>>>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first_line = String::new();
    reader.read_line(&mut first_line).unwrap();
    let path = first_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap();
        }
    }

    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).unwrap();
    }
    requests.lock().unwrap().push(RecordedRequest {
        path,
        body: String::from_utf8(body).unwrap(),
    });

    let response = responses
        .lock()
        .unwrap()
        .pop_front()
        .expect("mock response queue exhausted");
    let status_text = if response.status == 200 {
        "OK"
    } else {
        "ERROR"
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        status_text,
        response.body.len()
    )
    .unwrap();
    for (name, value) in response.headers {
        write!(stream, "{name}: {value}\r\n").unwrap();
    }
    write!(stream, "\r\n{}", response.body).unwrap();
}
