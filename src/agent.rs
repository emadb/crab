use serde_json::json;
use std::collections::BTreeMap;
use anyhow::{Context, Result};

use crate::{
    context::Conversation, conversation_entry, error::AgentError, message::{AssistantTurn, ToolResult}, provider::{LlmClient, TurnRequest}, tools::registry::ToolRegistry, ui::{AgentEvent, Ui},
};

pub struct Agent {
    llm: Box<dyn LlmClient>,
    tools: ToolRegistry,
    ui: Box<dyn Ui>,
    max_iterations: usize,
}

const COMPACT_PROMPT: &str = "The messages above are a conversation to summarize. Create a structured context checkpoint summary that another LLM will use to continue the work.

Use this EXACT format:

## Goal
[What is the user trying to accomplish? Can be multiple items if the session covers different tasks.]

## Constraints & Preferences
- [Any constraints, preferences, or requirements mentioned by user]
- [Or '(none)' if none were mentioned]

## Progress
### Done
- [x] [Completed tasks/changes]

### In Progress
- [ ] [Current work]

### Blocked
- [Issues preventing progress, if any]

## Key Decisions
- **[Decision]**: [Brief rationale]

## Next Steps
1. [Ordered list of what should happen next]

## Critical Context
- [Any data, examples, or references needed to continue]
- [Or '(none)' if not applicable]

Keep each section concise. Preserve exact file paths, function names, and error messages.";

impl Agent {
    pub fn new(llm: Box<dyn LlmClient>, tools: ToolRegistry, ui: Box<dyn Ui>) -> Self {
        Self {
            llm,
            tools,
            ui,
            max_iterations: 30,
        }
    }

    pub async fn run_turn(
        &self,
        conversation: &mut Conversation,
        input: &str,
    ) -> Result<()> {
        if conversation.needs_compression() {
            // TODO: manage error
            let _ = self.compact_conversation(conversation).await;
        }

        conversation.push_user(input);

        for _ in 0..self.max_iterations {
            let turn = self.request_turn(conversation).await?;

            let pending = conversation.push_assistant(turn);

            if completed {
                self.ui.emit(AgentEvent::TurnEnded);
                return Ok(());
            }

            for call in pending {
                let args = serde_json::from_str(call.arguments());
                let args = args.unwrap_or(json!({"error": "Error parsing the arguments."}));
                self.ui.emit(AgentEvent::ToolExecutionStarted {
                    name: String::from(call.name()),
                    arguments: args,
                });
                if self.ui.ask_permission() {
                    let result = self.execute(call.name(), call.arguments()).await;
                    self.ui.emit(AgentEvent::ToolFinished {
                        name: call.name().to_string(),
                        result: result.clone(),
                    });
                    conversation.resolve(call, result);
                }
            }
        }

        Err(AgentError::IterationLimit)
    }

    async fn execute(&self, name: &str, arguments: &str) -> ToolResult {
        let args: serde_json::Value = match serde_json::from_str(arguments) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult {
                    content: format!("invalid arguments: {e}"),
                    is_error: true,
                };
            }
        };

        match self.tools.get(name) {
            Some(tool) => match tool.execute(args).await {
                Ok(content) => ToolResult {
                    content,
                    is_error: false,
                },
                Err(e) => ToolResult {
                    content: e.to_string(),
                    is_error: true,
                },
            },
            None => ToolResult {
                content: format!("unknown tool: {name}"),
                is_error: true,
            },
        }
    }

    async fn compact_conversation(
        &self,
        conversation: &mut Conversation,
    ) -> Result<(), AgentError> {
        conversation.push_user(COMPACT_PROMPT);

        let turn = self.request_turn(conversation).await?;
        let _ = conversation.push_assistant(turn);
        self.ui.emit(AgentEvent::TurnEnded);

        Ok(())
    }

    async fn request_turn(&self, conversation: &Conversation) -> Result<crate::provider::openai::ConversationEntry> {
        let specs = self.tools.specs();
        let request = TurnRequest {
            messages: conversation.messages.as_slice(),
            tools: &specs,
        };
        let ui = &self.ui;
        let mut on_delta = |delta: crate::provider::openai::ChunkResponse| {
            // ui.emit(match delta {
            //     Delta::Text(text) => AgentEvent::TextDelta(text),
            //     Delta::ToolCallStarted { name } => AgentEvent::ToolStarted { name },
            // });
        };
        let conversation_entry = self.llm.send(request, &mut on_delta).await?;
        Ok()
    }
}
