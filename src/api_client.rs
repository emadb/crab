use std::io::Write;

use crate::{LlmConfig, message::Message};
use anyhow::{Result, bail};
use futures_util::{Stream, TryStreamExt, future, stream};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
    // reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: Delta,
}

#[derive(Deserialize)]
struct ChunkResponse {
    choices: Vec<ChunkChoice>,
}

pub async fn send_message(history: &[Message], llm_config: &LlmConfig) -> Result<String> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", llm_config.base_url);

    let response = client
        .post(&url)
        // .bearer_auth(api_key)
        .json(&ChatRequest {
            model: llm_config.model.to_string(),
            messages: history.to_vec(),
            stream: true,
        })
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("API error {status}: {body}");
    }

    let mut events = sse_events(response.bytes_stream());
    let mut full = String::new();
    let mut stdout = std::io::stdout();

    while let Some(data) = events.try_next().await? {
        let parsed = serde_json::from_str::<ChunkResponse>(&data)?;
        if let Some(token) = parsed
            .choices
            .first()
            .and_then(|c| c.delta.content.as_deref())
        {
            print!("{token}");
            stdout.flush()?;
            full.push_str(token);
        }
    }

    Ok(full)
}

/// Trasforma lo stream di byte HTTP in uno stream di payload `data:`,
/// che termina spontaneamente quando arriva `[DONE]`.
fn sse_events<B: AsRef<[u8]>>(
    bytes: impl Stream<Item = reqwest::Result<B>>,
) -> impl Stream<Item = Result<String>> {
    let mut parser = SseParser { buf: Vec::new() };
    bytes
        .map_ok(move |chunk| stream::iter(parser.push(chunk.as_ref()).into_iter().map(Ok)))
        .map_err(anyhow::Error::from)
        .try_flatten()
        .try_take_while(|data| future::ready(Ok(data != "[DONE]")))
}

struct SseParser {
    buf: Vec<u8>,
}

impl SseParser {
    /// Accumula un chunk HTTP, restituisce i payload `data:` completi.
    fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            if let Some(data) = line.trim().strip_prefix("data:") {
                out.push(data.trim().to_string());
            }
        }
        out
    }
}
