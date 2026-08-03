use crate::message::{AssistantTurn, Message, ToolResult};

pub struct Conversation {
    system: Option<String>,
    pub messages: Vec<Message>,
}

pub struct PendingCall {
    id: String,
    name: String,
    arguments: String,
}

impl PendingCall {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn arguments(&self) -> &str {
        &self.arguments
    }
}

impl Conversation {
    pub fn new(system: Option<String>) -> Self {
        let messages = system
            .clone()
            .map(|s| vec![Message::System(s)])
            .unwrap_or_default();
        Self { system, messages }
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        self.messages.push(Message::User(text.into()));
    }

    #[must_use]
    pub fn push_assistant(&mut self, turn: AssistantTurn) -> Vec<PendingCall> {
        match turn {
            AssistantTurn::Completed { text } => {
                self.messages.push(Message::Assistant {
                    text,
                    tool_calls: Vec::new(),
                });
                Vec::new()
            }
            AssistantTurn::ToolCalls { text, calls } => {
                let pending: Vec<PendingCall> = calls
                    .iter()
                    .map(|c| PendingCall {
                        id: c.id.clone(),
                        name: c.name.clone(),
                        arguments: c.arguments.clone(),
                    })
                    .collect();
                self.messages.push(Message::Assistant {
                    text,
                    tool_calls: calls,
                });
                pending
            }
        }
    }

    pub fn resolve(&mut self, call: PendingCall, result: ToolResult) {
        self.messages.push(Message::Tool {
            call_id: call.id,
            result,
        });
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.system.clone());
    }
}
