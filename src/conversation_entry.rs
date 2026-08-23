use crate::message::{Message, ToolCall, ToolResult};

#[derive(Debug)]
pub struct ConversationEntry {
    pub message: Message,
    pub tokens: usize,
    pub origin: Origin,
    pub compacted: bool,
}

impl ConversationEntry {
    pub fn system(system: Option<String>) -> Self {
        let (message, tokens) = match system {
            Some(s) => (Message::System(s.clone()), s.len() / 4),
            None => (Message::System(String::from("")), 0)
        };
        Self {
            message: message,
            tokens: tokens,
            origin: Origin::SystemPrompt,
            compacted: false
        }
    }
    pub fn user(text: String) -> Self {
        Self {
            message: Message::User(text.clone()),
            tokens: text.len() / 4,
            origin: Origin::UserInput,
            compacted: false
        }
    }
    pub fn agent(text: String, tool_calls: Vec<ToolCall>, tokens: usize) -> Self {
        Self {
            message: Message::Assistant{text: text.clone(), tool_calls: tool_calls},
            tokens: tokens,
            origin: Origin::ModelText,
            compacted: false
        }
    }
    pub fn tool_output(call_id: String, result: ToolResult, tool_name: String) -> Self {
        Self {
            message: Message::Tool { call_id, result: result.clone() },
            tokens: result.content.len() / 4,
            origin: Origin::ToolOutput(tool_name),
            compacted: false
        }
    }
}
#[derive(Debug)]
pub enum Origin {
    SystemPrompt,
    UserInput,
    ModelText,
    ToolOutput(String)
}