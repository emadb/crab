use crate::{context::Unresolved, provider::LlmError, tools::ToolError};

#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error(transparent)]
    Tool(#[from] ToolError),
    #[error("conversation has unresolved tool calls")]
    Unresolved,
    #[error("assistant turn truncated")]
    Truncated,
    #[error("reached max iterations")]
    IterationLimit,
}

impl From<Unresolved> for AgentError {
    fn from(_: Unresolved) -> Self {
        AgentError::Unresolved
    }
}
