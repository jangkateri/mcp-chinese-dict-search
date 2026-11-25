//! Stdio transport: newline-delimited JSON-RPC in, newline-delimited JSON-RPC
//! out, with diagnostics on stderr.
//!
//! Messages are handled concurrently so a `notifications/cancelled` can abort a
//! slow call instead of queueing behind it. [`serve`] returns only after the
//! input ends *and* every in-flight call has written its response, which keeps
//! piped smoke tests deterministic.

use std::sync::Arc;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use crate::logging::Logger;
use crate::protocol::{error_code, error_response};
use crate::server::McpServer;

/// Guard against a peer that never sends a newline: far above any real MCP message.
pub const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;

/// Serve the stdio transport until the input stream ends.
pub async fn serve<R, W>(
    server: Arc<McpServer>,
    input: R,
    output: W,
    logger: Logger,
) -> std::io::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let writer = Arc::new(Mutex::new(output));
    let mut reader = BufReader::new(input);
    let mut tasks: JoinSet<()> = JoinSet::new();
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        let read = reader.read_until(b'\n', &mut buffer).await?;
        if read == 0 {
            break;
        }

        if buffer.len() > MAX_LINE_BYTES {
            logger.error(format!(
                "dropping {} buffered bytes: no newline in sight",
                buffer.len()
            ));
            let response = error_response(
                None,
                error_code::PARSE_ERROR,
                "Parse error: message exceeded the line limit",
            );
            write_message(&writer, &response, &logger).await;
            continue;
        }

        let line = String::from_utf8_lossy(&buffer)
            .trim_end_matches(['\r', '\n'])
            .to_owned();
        if line.trim().is_empty() {
            continue;
        }

        let server = Arc::clone(&server);
        let writer = Arc::clone(&writer);
        let task_logger = logger.clone();
        tasks.spawn(async move {
            if let Some(response) = server.handle_line(&line).await {
                write_message(&writer, &response, &task_logger).await;
            }
        });
    }

    while tasks.join_next().await.is_some() {}
    server.abort_all();
    logger.debug("stdio input ended");
    Ok(())
}

/// Write one JSON-RPC message, ignoring a client that has already gone away.
async fn write_message<W>(writer: &Mutex<W>, message: &Value, logger: &Logger)
where
    W: AsyncWrite + Unpin,
{
    let mut line = match serde_json::to_vec(message) {
        Ok(line) => line,
        Err(error) => {
            logger.error(format!("cannot serialize a response: {error}"));
            return;
        }
    };
    line.push(b'\n');

    let mut guard = writer.lock().await;
    if let Err(error) = guard.write_all(&line).await {
        logger.debug(format!("write failed: {error}"));
        return;
    }
    if let Err(error) = guard.flush().await {
        logger.debug(format!("flush failed: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::io::{AsyncReadExt, duplex};

    use super::*;
    use crate::dictionary::Dictionary;

    #[tokio::test]
    async fn serves_requests_over_a_pipe_and_flushes_before_eof() {
        let (mut client, server_stream) = duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server_stream);
        let server = Arc::new(McpServer::new(
            Arc::new(Dictionary::empty()),
            Logger::new(false),
        ));

        let task = tokio::spawn(serve(
            Arc::clone(&server),
            server_read,
            server_write,
            Logger::new(false),
        ));

        client
            .write_all(
                b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\",\"params\":{}}\n\n\
                  {\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n",
            )
            .await
            .unwrap();
        client.shutdown().await.unwrap();

        let mut output = String::new();
        client.read_to_string(&mut output).await.unwrap();
        task.await.unwrap().unwrap();

        let lines: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["result"], json!({}));
        assert_eq!(lines[1]["result"]["tools"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn malformed_lines_get_parse_errors_and_processing_continues() {
        let (mut client, server_stream) = duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server_stream);
        let server = Arc::new(McpServer::new(
            Arc::new(Dictionary::empty()),
            Logger::new(false),
        ));

        let task = tokio::spawn(serve(
            Arc::clone(&server),
            server_read,
            server_write,
            Logger::new(false),
        ));

        client
            .write_all(b"{not json\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
            .await
            .unwrap();
        client.shutdown().await.unwrap();

        let mut output = String::new();
        client.read_to_string(&mut output).await.unwrap();
        task.await.unwrap().unwrap();

        let lines: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines[0]["error"]["code"], -32700);
        assert_eq!(lines[1]["result"], json!({}));
    }
}
