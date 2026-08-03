use crate::provider::LlmError;

#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error("reached max iterations")]
    IterationLimit,
}
