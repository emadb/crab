use crate::{conversation_entry::{ConversationEntry}, message::{AssistantTurn, ToolResult}};

#[derive(Debug)]
pub struct Conversation {
    system: Option<String>,
    pub messages: Vec<ConversationEntry>,
    context_size: u32,
    current_context: u32,
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
    pub fn new(system: Option<String>, context_size: u32) -> Self {
        let message = ConversationEntry::system(system.clone());
        Self { system, messages: vec![message], context_size, current_context: 0 }
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        let ce = ConversationEntry::user(text.into());
        if let Some(tokens) = ce.tokens {
            self.current_context += tokens as u32;
        }
        self.messages.push(ce);
    }

    #[must_use]
    pub fn push_assistant(&mut self, turn: AssistantTurn) -> Vec<PendingCall> {
        match turn {
            AssistantTurn::Completed { text, completion_tokens } => {
                let ce = ConversationEntry::agent(text, Vec::new(), completion_tokens);
                self.messages.push(ce);
                if let Some(tokens) = completion_tokens {
                    self.current_context += tokens as u32;
                }
                Vec::new()
            }
            AssistantTurn::ToolCalls { text, calls, completion_tokens } => {
                let pending: Vec<PendingCall> = calls
                    .iter()
                    .map(|c| PendingCall {
                        id: c.id.clone(),
                        name: c.name.clone(),
                        arguments: c.arguments.clone(),
                    })
                    .collect();
                if let Some(tokens) = completion_tokens {
                    self.current_context += tokens as u32;
                }
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
        *self = Self::new(self.system.clone(), self.context_size);
    }

    pub fn needs_compression(&self) -> bool {
        let threshold =  (0.8 * (self.context_size as f32)).floor() as u32;
        self.current_context > threshold
    }

    // fn verify_context_size(&self) {
    //     let threshold =  (0.8 * (self.context_size as f32)).floor() as u32;
    //     if self.current_context > threshold {
    //         // Sostituisco tutti i messaggi (Tranne il system prompt) con un messaggio riassuntivo
    //         // Chiedo all'AI di fare il riassunto e prendo il messaggio di risposta come sostituto
    //         // di tutta la conversazione
    //     }
    // }


    // fn verify_context_size(&self) {
    //     let threshold =  (0.8 * (self.context_size as f32)).floor() as u32;
    //     if self.current_context > threshold {
    //         let mut new_messages: Vec<ConversationEntry> = vec![];
    //         for m in self.messages.clone() {
    //             match m.origin {
    //                 Origin::ToolOutput(name) => {
    //                     let tr = ToolResult{
    //                         content: format!("[output di {name} rimosso]"),
    //                         is_error: false,
    //                     };
    //                     let entry = ConversationEntry::tool_output(String::from("removed"), tr, name);
    //                     new_messages.push(entry);
    //                 },
    //                 _ => {
    //                     new_messages.push(m);
    //                 },

    //             }

    //         }
    //     }
    // }
}
