pub mod render;

use crate::{error::AgentError, message::ToolResult};

pub enum AgentEvent {
    TextDelta(String),
    ToolStarted {
        name: String,
    },
    ToolExecutionStarted {
        name: String,
        arguments: serde_json::Value,
    },

    ToolFinished {
        name: String,
        result: ToolResult,
    },
    TurnEnded,
    // Not emitted yet: run_turn's Result is the only error path today, and main.rs
    // reports it. Kept on the enum for a future adapter (e.g. TUI) that wants to
    // render an error inline instead of waiting for the turn to return.
    #[allow(dead_code)]
    Error(AgentError),
}

pub trait Ui: Send + Sync {
    fn emit(&self, event: AgentEvent);
}
