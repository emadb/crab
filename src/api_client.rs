use crate::message::Message;
use futures_util::{Stream, StreamExt, TryStreamExt, future, stream};
use serde::{Deserialize, Serialize};
use std::{pin::Pin, time::Duration};

pub enum StreamEvent {
    TextDelta(String),
    // step 4: ToolCallDelta { index: usize, id: Option<String>, name: Option<String>, arguments: String },
    // i campi verranno letti allo step 8 (context management)
    Usage {
        #[allow(dead_code)]
        prompt_tokens: u32,
        #[allow(dead_code)]
        completion_tokens: u32,
    },
    Done,
}

pub type ChatStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>;

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn send_message(&self, messages: &[Message]) -> Result<ChatStream, LlmError>;
}

#[derive(thiserror::Error, Debug)]
pub enum LlmError {
    #[error("rate limit (429)")]
    RateLimited { retry_after: Option<Duration> },
    #[error("autenticazione: {0}")]
    Auth(String),
    #[error("errore API {status}: {body}")]
    Api { status: u16, body: String },
    #[error("rete: {0}")]
    Network(#[from] reqwest::Error),
    #[error("risposta malformata: {0}")]
    Parse(#[from] serde_json::Error),
}

impl LlmError {
    pub fn is_retryable(&self) -> bool {
        match self {
            LlmError::RateLimited { .. } | LlmError::Network(_) => true,
            LlmError::Api { status, .. } => (500..600).contains(status),
            LlmError::Auth(_) | LlmError::Parse(_) => false,
        }
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            LlmError::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    stream_options: StreamOptions,
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
struct UsageInfo {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Deserialize)]
struct ChunkResponse {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    usage: Option<UsageInfo>,
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
                stream_options: StreamOptions {
                    include_usage: true,
                },
            })
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Api { status: status.as_u16(), body });
        }

        let events = sse_events(response.bytes_stream())
            .try_filter_map(|data| future::ready(parse_chunk(&data)))
            .chain(stream::once(future::ready(Ok(StreamEvent::Done))));
        Ok(Box::pin(events))
    }
}

/// Un payload `data:` → al più uno StreamEvent (testo o usage).
fn parse_chunk(data: &str) -> Result<Option<StreamEvent>, LlmError> {
    let parsed: ChunkResponse = serde_json::from_str(data)?;
    if let Some(usage) = parsed.usage {
        return Ok(Some(StreamEvent::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
        }));
    }
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
