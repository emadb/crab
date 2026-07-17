use crate::message::Message;
use futures_util::{Stream, TryStreamExt, future, stream};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub enum StreamEvent {
    TextDelta(String),
    // step 4: ToolCallDelta { index: usize, id: Option<String>, name: Option<String>, arguments: String },
    // step 8: Usage { prompt_tokens: u32, completion_tokens: u32 },
}

pub type ChatStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>;

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn send_message(&self, messages: &[Message]) -> Result<ChatStream, LlmError>;
}

#[derive(thiserror::Error, Debug)]
pub enum LlmError {
    #[error("API error {status}: {body}")]
    Api { status: u16, body: String },
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error("malformed response: {0}")]
    Parse(#[from] serde_json::Error),
}

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

pub struct ChatServer {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl ChatServer {
    pub fn new(base_url: String, model: String) -> Self {
        let client = reqwest::Client::new();
        Self {
            client,
            base_url,
            model,
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for ChatServer {
    async fn send_message(&self, history: &[Message]) -> Result<ChatStream, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            // .bearer_auth(api_key)
            .json(&ChatRequest {
                model: self.model.to_string(),
                messages: history.to_vec(),
                stream: true,
            })
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let events = sse_events(response.bytes_stream())
            .try_filter_map(|data| future::ready(parse_chunk(&data)));
        Ok(Box::pin(events))
    }
}

/// Un payload `data:` → al più uno StreamEvent (i chunk senza testo vengono scartati).
fn parse_chunk(data: &str) -> Result<Option<StreamEvent>, LlmError> {
    let parsed: ChunkResponse = serde_json::from_str(data)?;
    Ok(parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.delta.content)
        .map(StreamEvent::TextDelta))
}

/// Trasforma lo stream di byte HTTP in uno stream di payload `data:`,
/// che termina spontaneamente quando arriva `[DONE]`.
fn sse_events<B: AsRef<[u8]>>(
    bytes: impl Stream<Item = reqwest::Result<B>>,
) -> impl Stream<Item = Result<String, LlmError>> {
    let mut parser = SseParser { buf: Vec::new() };
    bytes
        .map_ok(move |chunk| stream::iter(parser.push(chunk.as_ref()).into_iter().map(Ok)))
        .map_err(LlmError::from)
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
