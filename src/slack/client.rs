use std::thread;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::{Client as HttpClient, RequestBuilder};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, RETRY_AFTER};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{Error, Result};

#[derive(Clone)]
pub struct SlackClient {
    http: HttpClient,
    token: String,
    base_url: String,
    rate_limit_retries: u32,
}

impl SlackClient {
    pub fn new(token: String, base_url: String, rate_limit_retries: u32) -> Result<Self> {
        let http = HttpClient::builder()
            .build()
            .map_err(|error| Error::SlackHttp {
                method: "client",
                message: error.to_string(),
            })?;

        Ok(Self {
            http,
            token,
            base_url: base_url.trim_end_matches('/').to_owned(),
            rate_limit_retries,
        })
    }

    pub fn get<T: DeserializeOwned>(
        &self,
        method: &'static str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = self.url(method);
        self.send(method, || {
            self.http
                .get(&url)
                .headers(self.auth_headers())
                .query(query)
        })
    }

    pub fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        method: &'static str,
        body: &B,
    ) -> Result<T> {
        let url = self.url(method);
        self.send(method, || {
            self.http.post(&url).headers(self.auth_headers()).json(body)
        })
    }

    fn send<T: DeserializeOwned, F: FnMut() -> RequestBuilder>(
        &self,
        method: &'static str,
        mut build: F,
    ) -> Result<T> {
        let mut attempt = 0;

        loop {
            let response = build().send().map_err(|error| Error::SlackHttp {
                method,
                message: error.to_string(),
            })?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS
                && attempt < self.rate_limit_retries
            {
                let retry_after = retry_after(response.headers());
                attempt += 1;
                thread::sleep(retry_after);
                continue;
            }

            let status = response.status();
            let body = response.text().map_err(|error| Error::SlackHttp {
                method,
                message: error.to_string(),
            })?;

            if !status.is_success() {
                return Err(Error::SlackHttpStatus {
                    method,
                    status: status.as_u16(),
                    body,
                });
            }

            let value: Value =
                serde_json::from_str(&body).map_err(|error| Error::MalformedSlackResponse {
                    method,
                    message: error.to_string(),
                })?;

            if value.get("ok").and_then(Value::as_bool) == Some(false) {
                let error = value
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown_error")
                    .to_owned();
                return Err(Error::SlackApi {
                    method,
                    error,
                    details: slack_error_details(&value),
                });
            }

            return serde_json::from_value(value).map_err(|error| Error::MalformedSlackResponse {
                method,
                message: error.to_string(),
            });
        }
    }

    fn url(&self, method: &str) -> String {
        format!("{}/{}", self.base_url, method)
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let value = HeaderValue::from_str(&format!("Bearer {}", self.token))
            .expect("Slack token should be representable as a header value");
        headers.insert(AUTHORIZATION, value);
        headers
    }
}

fn slack_error_details(value: &Value) -> String {
    let mut details = [
        ("needed", value.get("needed").and_then(Value::as_str)),
        ("provided", value.get("provided").and_then(Value::as_str)),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.map(|value| format!("{label}: {value}")))
    .collect::<Vec<_>>();

    if value.get("error").and_then(Value::as_str) == Some("missing_scope")
        && value
            .get("provided")
            .and_then(Value::as_str)
            .is_some_and(|scopes| scopes.contains("app_configurations:"))
    {
        details.push(
            "hint: this token appears to be a Slack App Configuration Token; use a Slack Web API user token with the required search scopes"
                .to_owned(),
        );
    }

    if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join("; "))
    }
}

fn retry_after(headers: &HeaderMap) -> Duration {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(1))
}
