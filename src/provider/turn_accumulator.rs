use serde::Deserialize;

use crate::{message::{AssistantTurn, ToolCall}, provider::{Delta, LlmError, openai::UsageInfo}};

#[derive(Default)]
pub struct TurnAccumulator {
    text: String,
    calls: Vec<ToolCall>,
    usage: Option<UsageInfo>,
    finished: bool,
}

impl TurnAccumulator {
    pub fn apply(&mut self, response: ChunkResponse) -> Vec<Delta> {
        if response.usage.is_some() {
            self.usage = response.usage;
        }

        let mut events = Vec::new();
        for choice in response.choices {
            self.finished |= choice.finish_reason.is_some();
            events.extend(self.apply_delta(choice.delta));
        }

        events
    }

    fn apply_delta(&mut self, delta: DeltaChunk) -> Vec<Delta> {
        let mut events = Vec::new();

        if let Some(text) = delta.content {
            self.text.push_str(&text);
            events.push(Delta::Text(text));
        }

        for chunk in delta.tool_calls.unwrap_or_default() {
            while self.calls.len() <= chunk.index {
               self.calls.push(TurnAccumulator::empty_call());
            }
            let call = &mut self.calls[chunk.index];

            if let Some(id) = chunk.id {
                call.id = id;
            }

            if let Some(function) = chunk.function {
                if let Some(name) = function.name {
                    if call.name.is_empty() {
                        events.push(Delta::ToolCallStarted { name: name.clone() });
                    }
                    call.name.push_str(&name);
                }
                call.arguments.push_str(&function.arguments);
            }
        }

        events
    }

    pub fn finish(self) -> Result<AssistantTurn, LlmError> {
        if !self.finished {
            return Err(LlmError::IncompleteStream);
        }

        let completion_tokens = self.usage.map(|usage| usage.completion_tokens);

        Ok(if self.calls.is_empty() {
            AssistantTurn::Completed {
                text: self.text,
                completion_tokens,
            }
        } else {
            AssistantTurn::ToolCalls {
                text: self.text,
                calls: self.calls,
                completion_tokens,
            }
        })
    }
    fn empty_call() -> ToolCall {
        ToolCall {
            id: String::new(),
            name: String::new(),
            arguments: String::new(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct ChunkResponse {
    choices: Vec<ChunkChoice>,
    usage: Option<UsageInfo>,
}

#[derive(Deserialize, Debug)]
struct ChunkChoice {
    delta: DeltaChunk,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DeltaChunk {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallChunk>>,
}

#[derive(Deserialize, Debug)]
struct ToolCallChunk {
    index: usize,
    id: Option<String>,
    function: Option<FunctionChunk>,
}

#[derive(Deserialize, Debug)]
struct FunctionChunk {
    name: Option<String>,
    #[serde(default)]
    arguments: String,
}