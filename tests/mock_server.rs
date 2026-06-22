use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Output, Stdio};

#[test]
fn server_command_backs_search_cli() {
    let server = RunningServer::start();
    let tempdir = tempfile::tempdir().unwrap();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &server.url,
        "--page-size",
        "1",
        "--max-results",
        "10",
        "--text",
        "roadmap",
    ]);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("user_name: Ada Lovelace"));
    assert!(stdout.contains("text: The roadmap is ready."));
    assert!(stdout.contains("user_name: Grace Hopper"));
    assert!(stdout.contains("text: The roadmap follow-up is scheduled."));
}

#[test]
fn server_command_supports_resolution_and_legacy_fallback() {
    let server = RunningServer::start();
    let tempdir = tempfile::tempdir().unwrap();
    let config = write_config(tempdir.path());

    let output = run_slacksearch([
        "search",
        "--config",
        config.to_str().unwrap(),
        "--api-base-url",
        &server.url,
        "--search-channel",
        "general",
        "--search-chat",
        "John Doe",
        "--text",
        "force legacy",
    ]);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("user_name: Ada Lovelace"));
    assert!(stdout.contains("text: The legacy roadmap result is ready."));
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

struct RunningServer {
    url: String,
    child: Child,
}

impl RunningServer {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_slacksearch"))
            .args(["server", "--bind", "127.0.0.1:0"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();

        let url = line
            .split_whitespace()
            .last()
            .expect("server should print listening URL")
            .to_owned();

        Self { url, child }
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
