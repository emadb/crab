use crate::message::{AssistantTurn, Message, ToolResult};

pub struct Conversation {
    system: Option<String>,
    messages: Vec<Message>,
    pending: Vec<String>,
}

pub struct PendingCall {
    id: String,
    name: String,
    arguments: String,
}

pub struct Unresolved;

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
        Self {
            system,
            messages,
            pending: Vec::new(),
        }
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        self.messages.push(Message::User(text.into()));
    }

    #[must_use]
    pub fn push_assistant(&mut self, turn: AssistantTurn) -> Vec<PendingCall> {
        match turn {
            AssistantTurn::Completed { text } | AssistantTurn::Truncated { text } => {
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
                self.pending = pending.iter().map(|p| p.id.clone()).collect();
                self.messages.push(Message::Assistant {
                    text,
                    tool_calls: calls,
                });
                pending
            }
        }
    }

    pub fn resolve(&mut self, call: PendingCall, result: ToolResult) {
        if let Some(pos) = self.pending.iter().position(|id| id == &call.id) {
            self.pending.remove(pos);
        }
        self.messages.push(Message::Tool {
            call_id: call.id,
            result,
        });
    }

    pub fn messages(&self) -> Result<&[Message], Unresolved> {
        if self.pending.is_empty() {
            Ok(&self.messages)
        } else {
            Err(Unresolved)
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.system.clone());
    }
}
