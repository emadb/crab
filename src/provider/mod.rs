pub mod openai;

use crate::{provider::openai::{ChunkResponse, ConversationEntry}, tools::ToolSpec};

pub struct TurnRequest<'a> {
    pub messages: &'a [ConversationEntry],
    pub tools: &'a [ToolSpec],
}

#[derive(thiserror::Error, Debug)]
pub enum LlmError {
    #[error("API error {status}: {body}")]
    Api { status: u16, body: String },
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error("malformed response: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("malformed SSE UTF-8: {0}")]
    SseUtf8(#[from] std::string::FromUtf8Error),
    #[error("stream ended before the model completed the turn")]
    IncompleteStream,
}

#[async_trait::async_trait(?Send)]
pub trait LlmClient: Send + Sync {
    async fn send(
        &self,
        req: TurnRequest<'_>,
        on_delta: &mut (dyn FnMut(ChunkResponse) + Send),
    ) -> anyhow::Result<openai::ConversationEntry>;
}
