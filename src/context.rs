use crate::{conversation_entry::ConversationEntry, message::{AssistantTurn, ToolResult}};

#[derive(Debug)]
pub struct Conversation {
    system: Option<String>,
    pub messages: Vec<ConversationEntry>,
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
        let message = ConversationEntry::system(system.clone());
        Self { system, messages: vec![message] }
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        let ce = ConversationEntry::user(text.into());
        self.messages.push(ce);
    }

    #[must_use]
    pub fn push_assistant(&mut self, turn: AssistantTurn) -> Vec<PendingCall> {
        match turn {
            AssistantTurn::Completed { text, prompt_tokens, completion_tokens } => {
                let last = self.messages.last_mut();
                if let Some(m) = last { m.tokens = prompt_tokens }

                let ce = ConversationEntry::agent(text, Vec::new(), completion_tokens);
                self.messages.push(ce);

                Vec::new()
            }
            AssistantTurn::ToolCalls { text, calls, prompt_tokens, completion_tokens } => {
                let last = self.messages.last_mut();
                if let Some(m) = last { m.tokens = prompt_tokens }

                let pending: Vec<PendingCall> = calls
                    .iter()
                    .map(|c| PendingCall {
                        id: c.id.clone(),
                        name: c.name.clone(),
                        arguments: c.arguments.clone(),
                    })
                    .collect();

                let ce = ConversationEntry::agent(text.clone(), calls, completion_tokens);
                self.messages.push(ce);
                pending
            }
        }
    }

    pub fn resolve(&mut self, call: PendingCall, result: ToolResult) {
        let ce = ConversationEntry::tool_output(call.id, result, call.name);
        self.messages.push(ce);
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.system.clone());
    }
}
