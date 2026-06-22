use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

use crate::error::{Error, Result};
use crate::mock_slack::{self, MockHttpResponse};

pub fn run(bind: &str) -> Result<()> {
    let listener = TcpListener::bind(bind).map_err(|source| Error::MockServerBind {
        bind: bind.to_owned(),
        source,
    })?;
    let addr = listener.local_addr().map_err(Error::MockServerIo)?;

    println!("mock Slack API server listening on http://{addr}");
    std::io::stdout().flush().map_err(Error::MockServerIo)?;

    for stream in listener.incoming() {
        let stream = stream.map_err(Error::MockServerIo)?;
        handle_connection(stream)?;
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone().map_err(Error::MockServerIo)?);
    let mut first_line = String::new();
    reader
        .read_line(&mut first_line)
        .map_err(Error::MockServerIo)?;

    let mut request_parts = first_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default();
    let target = request_parts.next().unwrap_or_default();
    if method.is_empty() || target.is_empty() {
        write_response(
            &mut stream,
            MockHttpResponse {
                status: 400,
                content_type: "application/json",
                body: r#"{"ok":false,"error":"bad_request"}"#.to_owned(),
            },
        )?;
        return Ok(());
    }

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(Error::MockServerIo)?;
        if line == "\r\n" || line.is_empty() {
            break;
        }

        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse::<usize>().unwrap_or(0);
        }
    }

    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).map_err(Error::MockServerIo)?;
    }
    let body = String::from_utf8_lossy(&body);
    let response = mock_slack::handle(method, target, &body);
    write_response(&mut stream, response)
}

fn write_response(stream: &mut TcpStream, response: MockHttpResponse) -> Result<()> {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Error",
    };

    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.status,
        reason,
        response.content_type,
        response.body.len(),
        response.body
    )
    .map_err(Error::MockServerIo)
}
