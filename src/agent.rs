use serde_json::json;

use crate::{
    context::Conversation,
    error::AgentError,
    message::{AssistantTurn, ToolResult},
    provider::{Delta, LlmClient, TurnRequest},
    tools::registry::ToolRegistry,
    ui::{AgentEvent, Ui},
};

pub struct Agent {
    llm: Box<dyn LlmClient>,
    tools: ToolRegistry,
    ui: Box<dyn Ui>,
    max_iterations: usize,
}

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
    ) -> Result<(), AgentError> {
        conversation.push_user(input);

        for _ in 0..self.max_iterations {
            let specs = self.tools.specs();
            let messages = conversation.messages.as_slice();
            let request = TurnRequest {
                messages,
                tools: &specs,
            };

            let ui = &self.ui;
            let mut on_delta = |delta: Delta| {
                ui.emit(match delta {
                    Delta::Text(text) => AgentEvent::TextDelta(text),
                    Delta::ToolCallStarted { name } => AgentEvent::ToolStarted { name },
                });
            };

            let turn = match self.llm.send(request, &mut on_delta).await {
                Ok(turn) => turn,
                Err(e) => return Err(e.into()),
            };

            let completed = matches!(turn, AssistantTurn::Completed { .. });

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
}
