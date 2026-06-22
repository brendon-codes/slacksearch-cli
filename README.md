# Slack Search CLI

`slacksearch` is a Rust CLI foundation for searching Slack messages from the terminal.

## Build

```sh
cargo build
```

The binary is named `slacksearch`.

## Commands

```sh
slacksearch search --before '2026-05-01 12:31:01-06' "find this phrase in all chats and channels"
slacksearch search --after '2026-03-01' --before '2026-05-01' --search-channel general-discussion "find this phrase"
slacksearch search --search-chat 'John Doe' "find this phrase"
slacksearch search --json "find this phrase"
slacksearch search --text "find this phrase"
slacksearch search --context-msg-cnt 2 --json "find this phrase"
slacksearch search --config ./slacksearch.jsonc --max-results 25 --page-size 10 "find this phrase"
slacksearch search --api-base-url http://127.0.0.1:3000 "find this phrase"
slacksearch make-config
slacksearch make-config --force
slacksearch validate-config
slacksearch auth
slacksearch server
```

Search execution is implemented against Slack's modern `assistant.search.context` API by default, with automatic fallback support for legacy `search.messages` when the modern API is unavailable.

## Config

The default config file path is:

```text
~/.slacksearch/slacksearch.jsonc
```

`slacksearch make-config` creates a token placeholder config and refuses to overwrite an existing file unless `--force` is passed. Repository example configuration lives at `examples/.slacksearch/slacksearch.jsonc`.

`slacksearch validate-config` checks that the config parses as JSONC and contains a Slack Web API token at top-level `slack_web_api_token`.

`slack_web_api_token` is the only supported config value. Search uses `https://slack.com/api`, automatic assistant-to-legacy fallback, page size `20`, max results `100`, and one rate-limit retry by default. `slacksearch search` accepts `--page-size`, `--max-results`, and `--api-base-url` overrides for a single run. Commands that read or write config accept `--config`.

## Auth

`slacksearch auth` prompts for a Slack Web API access token and stores it locally as `slack_web_api_token` in the config file. Use a user token for broad user-visible Slack search across public channels, private channels, DMs, and group DMs.

Slack App Configuration Tokens are not Slack Web API access tokens and should not be pasted into `slacksearch auth`.

## Mock Server

`slacksearch server` runs a deterministic local Slack API mock for development and tests:

```sh
slacksearch server --bind 127.0.0.1:3000
```

Pass the printed `http://host:port` URL to `slacksearch search --api-base-url` and use any non-empty test token such as `xoxp-test-token` in `slack_web_api_token`. The mock supports `/health` and the Slack API paths used by search: `assistant.search.context`, `search.messages`, `conversations.list`, and `users.list`.

## Search

`slacksearch search` supports `--before`, `--after`, `--search-channel`, and `--search-chat`. Date-only filters use UTC midnight; timestamp filters such as `2026-05-01 12:31:01-06` preserve the supplied numeric offset.

Channel names are resolved through `conversations.list`. Chat/person names are resolved through `users.list`; ambiguous names fail instead of choosing a user silently.

Search output defaults to JSON. Use `--text`, `--json`, or `--markdown` to choose a format for a single run:

```sh
slacksearch search --text "roadmap"
slacksearch search --json "roadmap"
slacksearch search --markdown "roadmap"
```

Use `--context-msg-cnt N` to include up to N messages before and up to N messages after each assistant search result when Slack returns context.

JSON is the hard default for new searches. This intentionally supersedes the stale text-default requirement in `plans/initial/02-slack-search-behavior.md`.

## Quality Checks

Run the same checks used by CI and local `prek` hooks:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
prek run --all-files
```
