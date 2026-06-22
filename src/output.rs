use serde::Serialize;

use crate::cli::OutputFormat;
use crate::error::Result;
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct SearchResults {
    pub query: String,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct SearchResult {
    pub channel_id: Option<String>,
    pub channel_name: Option<String>,
    pub user_id: Option<String>,
    pub user_name: Option<String>,
    pub timestamp: String,
    pub text: String,
    pub permalink: Option<String>,
    pub before: Vec<SearchMessage>,
    pub after: Vec<SearchMessage>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct SearchMessage {
    pub channel_id: Option<String>,
    pub channel_name: Option<String>,
    pub user_id: Option<String>,
    pub user_name: Option<String>,
    pub timestamp: String,
    pub text: String,
    pub permalink: Option<String>,
}

pub fn render(results: &SearchResults, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Text => Ok(render_text(results)),
        OutputFormat::Json => {
            Ok(serde_json::to_string_pretty(results).expect("search results should serialize"))
        }
        OutputFormat::Markdown => Ok(render_markdown(results)),
    }
}

fn render_text(results: &SearchResults) -> String {
    if results.results.is_empty() {
        return "No results found.\n".to_owned();
    }

    let mut output = format!("query: {}\n", one_line(&results.query));
    for (index, result) in results.results.iter().enumerate() {
        output.push_str(&format!("\nresult {}:\n", index + 1));
        push_message_text(&mut output, result.as_message(), "  ");
        push_context_text(&mut output, "before", &result.before);
        push_context_text(&mut output, "after", &result.after);
    }
    output
}

fn render_markdown(results: &SearchResults) -> String {
    if results.results.is_empty() {
        return "No results found.\n".to_owned();
    }

    let mut output = format!("query: `{}`\n", markdown_value(&results.query));
    for (index, result) in results.results.iter().enumerate() {
        output.push_str(&format!("\n## Result {}\n\n", index + 1));
        push_message_markdown(&mut output, result.as_message());
        push_context_markdown(&mut output, "Before", &result.before);
        push_context_markdown(&mut output, "After", &result.after);
    }
    output
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl SearchResult {
    fn as_message(&self) -> SearchMessage {
        SearchMessage {
            channel_id: self.channel_id.clone(),
            channel_name: self.channel_name.clone(),
            user_id: self.user_id.clone(),
            user_name: self.user_name.clone(),
            timestamp: self.timestamp.clone(),
            text: self.text.clone(),
            permalink: self.permalink.clone(),
        }
    }
}

fn push_context_text(output: &mut String, label: &str, messages: &[SearchMessage]) {
    if messages.is_empty() {
        return;
    }

    output.push_str(&format!("  {label}:\n"));
    for (index, message) in messages.iter().enumerate() {
        output.push_str(&format!("    message {}:\n", index + 1));
        push_message_text(output, message.clone(), "      ");
    }
}

fn push_message_text(output: &mut String, message: SearchMessage, indent: &str) {
    push_optional_text(output, indent, "channel_id", message.channel_id.as_deref());
    push_optional_text(
        output,
        indent,
        "channel_name",
        message.channel_name.as_deref(),
    );
    push_optional_text(output, indent, "user_id", message.user_id.as_deref());
    push_optional_text(output, indent, "user_name", message.user_name.as_deref());
    output.push_str(&format!("{indent}timestamp: {}\n", message.timestamp));
    output.push_str(&format!("{indent}text: {}\n", one_line(&message.text)));
    push_optional_text(output, indent, "permalink", message.permalink.as_deref());
}

fn push_optional_text(output: &mut String, indent: &str, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        output.push_str(&format!("{indent}{label}: {}\n", one_line(value)));
    }
}

fn push_context_markdown(output: &mut String, label: &str, messages: &[SearchMessage]) {
    if messages.is_empty() {
        return;
    }

    output.push_str(&format!("\n### {label}\n\n"));
    for (index, message) in messages.iter().enumerate() {
        output.push_str(&format!("Message {}\n\n", index + 1));
        push_message_markdown(output, message.clone());
    }
}

fn push_message_markdown(output: &mut String, message: SearchMessage) {
    output.push_str("| Field | Value |\n| --- | --- |\n");
    push_optional_markdown(output, "channel_id", message.channel_id.as_deref());
    push_optional_markdown(output, "channel_name", message.channel_name.as_deref());
    push_optional_markdown(output, "user_id", message.user_id.as_deref());
    push_optional_markdown(output, "user_name", message.user_name.as_deref());
    output.push_str(&format!(
        "| timestamp | {} |\n",
        markdown_value(&message.timestamp)
    ));
    output.push_str(&format!("| text | {} |\n", markdown_value(&message.text)));
    push_optional_markdown(output, "permalink", message.permalink.as_deref());
}

fn push_optional_markdown(output: &mut String, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        output.push_str(&format!("| {label} | {} |\n", markdown_value(value)));
    }
}

fn markdown_value(value: &str) -> String {
    one_line(value).replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use crate::cli::OutputFormat;
    use crate::output::{SearchMessage, SearchResult, SearchResults, render};

    fn sample_results() -> SearchResults {
        SearchResults {
            query: "roadmap".to_owned(),
            results: vec![SearchResult {
                channel_id: Some("C123".to_owned()),
                channel_name: Some("general".to_owned()),
                user_id: Some("U123".to_owned()),
                user_name: Some("Ada".to_owned()),
                timestamp: "1508284197.000015".to_owned(),
                text: "Ship it\nsoon".to_owned(),
                permalink: Some("https://example.slack.com/archives/C123/p1".to_owned()),
                before: vec![SearchMessage {
                    channel_id: Some("C123".to_owned()),
                    channel_name: Some("general".to_owned()),
                    user_id: Some("U456".to_owned()),
                    user_name: Some("Grace".to_owned()),
                    timestamp: "1508284097.000015".to_owned(),
                    text: "Earlier context".to_owned(),
                    permalink: Some("https://example.slack.com/archives/C123/p0".to_owned()),
                }],
                after: vec![SearchMessage {
                    channel_id: Some("C123".to_owned()),
                    channel_name: Some("general".to_owned()),
                    user_id: Some("U789".to_owned()),
                    user_name: Some("Linus".to_owned()),
                    timestamp: "1508284297.000015".to_owned(),
                    text: "Later context".to_owned(),
                    permalink: None,
                }],
            }],
        }
    }

    #[test]
    fn renders_text_output() {
        let rendered = render(&sample_results(), OutputFormat::Text).unwrap();

        assert!(rendered.contains("query: roadmap"));
        assert!(rendered.contains("channel_id: C123"));
        assert!(rendered.contains("user_name: Ada"));
        assert!(rendered.contains("text: Ship it soon"));
        assert!(rendered.contains("before:"));
        assert!(rendered.contains("text: Earlier context"));
        assert!(rendered.contains("after:"));
        assert!(rendered.contains("text: Later context"));
    }

    #[test]
    fn renders_json_output() {
        let rendered = render(&sample_results(), OutputFormat::Json).unwrap();

        assert!(rendered.contains("\"query\": \"roadmap\""));
        assert!(rendered.contains("\"channel_name\": \"general\""));
        assert!(rendered.contains("\"before\": ["));
        assert!(rendered.contains("\"after\": ["));
        assert!(rendered.contains("\"text\": \"Earlier context\""));
    }

    #[test]
    fn renders_markdown_output() {
        let rendered = render(&sample_results(), OutputFormat::Markdown).unwrap();

        assert!(rendered.contains("query: `roadmap`"));
        assert!(rendered.contains("## Result 1"));
        assert!(rendered.contains("| user_name | Ada |"));
        assert!(rendered.contains("### Before"));
        assert!(rendered.contains("| text | Earlier context |"));
    }

    #[test]
    fn renders_empty_results() {
        let rendered = render(
            &SearchResults {
                query: "missing".to_owned(),
                results: Vec::new(),
            },
            OutputFormat::Text,
        )
        .unwrap();

        assert_eq!(rendered, "No results found.\n");
    }
}
