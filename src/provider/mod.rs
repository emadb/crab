pub mod openai;
pub mod sse;

use crate::{
    message::{AssistantTurn, Message},
    tools::ToolSpec,
};

pub struct TurnRequest<'a> {
    pub messages: &'a [Message],
    pub tools: &'a [ToolSpec],
}

pub enum Delta {
    Text(String),
    ToolCallStarted { name: String },
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

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn send(
        &self,
        req: TurnRequest<'_>,
        on_delta: &mut (dyn FnMut(Delta) + Send),
    ) -> Result<AssistantTurn, LlmError>;
}
