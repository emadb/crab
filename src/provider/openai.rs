use crate::{
    message::{AssistantTurn, Message, ToolCall},
    provider::{Delta, LlmClient, LlmError, TurnRequest, sse::sse_events},
    tools::ToolSpec,
};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<MessageReq>,
    stream: bool,
    tools: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
}

#[derive(Serialize)]
struct MessageReq {
    role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<WireToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
struct WireToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireFunction,
}

#[derive(Serialize)]
struct WireFunction {
    name: String,
    arguments: String,
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

#[derive(Deserialize, Debug)]
struct ChunkChoice {
    delta: DeltaChunk,
}

#[derive(Deserialize, Debug)]
struct ChunkResponse {
    choices: Vec<ChunkChoice>,
    usage: Option<UsageInfo>,
}

#[derive(serde::Deserialize, Debug)]
pub struct UsageInfo {
    // pub prompt_tokens: usize,
    // pub total_tokens: usize,
    pub completion_tokens: usize,
}

#[derive(Default)]
struct TurnAccumulator {
    text: String,
    calls: Vec<ToolCall>,
    usage: Option<UsageInfo>,
}

impl TurnAccumulator {
    fn apply(&mut self, response: ChunkResponse) -> Vec<Delta> {
        if response.usage.is_some() {
            self.usage = response.usage;
        }

        response
            .choices
            .into_iter()
            .flat_map(|choice| self.apply_delta(choice.delta))
            .collect()
    }

    fn apply_delta(&mut self, delta: DeltaChunk) -> Vec<Delta> {
        let mut events = Vec::new();

        if let Some(text) = delta.content {
            self.text.push_str(&text);
            events.push(Delta::Text(text));
        }

        for chunk in delta.tool_calls.unwrap_or_default() {
            if self.calls.len() <= chunk.index {
                self.calls.resize_with(chunk.index + 1, empty_call);
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

    fn finish(self) -> AssistantTurn {
        let completion_tokens = self.usage.map(|usage| usage.completion_tokens);

        if self.calls.is_empty() {
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
        }
    }
}

fn empty_call() -> ToolCall {
    ToolCall {
        id: String::new(),
        name: String::new(),
        arguments: String::new(),
    }
}

fn decode_chunk(data: &str) -> Result<ChunkResponse, LlmError> {
    Ok(serde_json::from_str(data)?)
}

pub struct OpenAiClient {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl OpenAiClient {
    pub fn new(base_url: String, model: String, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            model,
            api_key,
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for OpenAiClient {
    async fn send(
        &self,
        req: TurnRequest<'_>,
        on_delta: &mut (dyn FnMut(Delta) + Send),
    ) -> Result<AssistantTurn, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut request = self
            .client
            .post(&url)
            .json(&build_request(&self.model, req));

        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }

        let response = request.send().await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let mut events = Box::pin(sse_events(response.bytes_stream()));
        let mut turn = TurnAccumulator::default();

        while let Some(data) = events.try_next().await? {
            for delta in turn.apply(decode_chunk(&data)?) {
                on_delta(delta);
            }
        }

        Ok(turn.finish())
    }
}

fn build_request(model: &str, request: TurnRequest<'_>) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: request
            .messages
            .iter()
            .map(|entry| build_message(&entry.message))
            .collect(),
        stream: true,
        tools: tools_json(request.tools),
        stream_options: Some(StreamOptions {
            include_usage: true,
        }),
    }
}

fn build_message(message: &Message) -> MessageReq {
    match message {
        Message::System(text) => MessageReq {
            role: "system",
            content: Some(text.clone()),
            tool_calls: None,
            tool_call_id: None,
        },
        Message::User(text) => MessageReq {
            role: "user",
            content: Some(text.clone()),
            tool_calls: None,
            tool_call_id: None,
        },
        Message::Assistant { text, tool_calls } => MessageReq {
            role: "assistant",
            content: if text.is_empty() {
                None
            } else {
                Some(text.clone())
            },
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(
                    tool_calls
                        .iter()
                        .map(|c| WireToolCall {
                            id: c.id.clone(),
                            kind: "function",
                            function: WireFunction {
                                name: c.name.clone(),
                                arguments: c.arguments.clone(),
                            },
                        })
                        .collect(),
                )
            },
            tool_call_id: None,
        },
        Message::Tool { call_id, result } => MessageReq {
            role: "tool",
            content: Some(result.content.clone()),
            tool_calls: None,
            tool_call_id: Some(call_id.clone()),
        },
    }
}

fn tools_json(specs: &[ToolSpec]) -> serde_json::Value {
    let jtool: Vec<_> = specs
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })
        })
        .collect();
    serde_json::json!(jtool)
}

#[cfg(test)]
mod tests {
    use super::{TurnAccumulator, decode_chunk};
    use crate::{message::AssistantTurn, provider::Delta};

    #[test]
    fn accumulates_text_usage_and_multiple_tool_calls() {
        let chunks = [
            r#"{"choices":[{"delta":{"content":"I ","tool_calls":[{"index":0,"id":"call_1","function":{"name":"read_","arguments":"{\"path\":\""}},{"index":1,"id":"call_2","function":{"name":"ls","arguments":"{\"path\":\""}}]}}]}"#,
            r#"{"choices":[{"delta":{"content":"found","tool_calls":[{"index":0,"function":{"name":"file","arguments":"src/main.rs\"}"}},{"index":1,"function":{"arguments":"src\"}"}}]}}]}"#,
            r#"{"choices":[],"usage":{"completion_tokens":12}}"#,
        ];
        let mut accumulator = TurnAccumulator::default();
        let mut events = Vec::new();

        for chunk in chunks {
            let response =
                decode_chunk(chunk).unwrap_or_else(|error| panic!("invalid JSON: {error}"));
            events.extend(accumulator.apply(response));
        }

        assert!(matches!(events[0], Delta::Text(ref text) if text == "I "));
        assert!(matches!(events[1], Delta::ToolCallStarted { ref name } if name == "read_"));
        assert!(matches!(events[2], Delta::ToolCallStarted { ref name } if name == "ls"));
        assert!(matches!(events[3], Delta::Text(ref text) if text == "found"));

        match accumulator.finish() {
            AssistantTurn::ToolCalls {
                text,
                calls,
                completion_tokens,
            } => {
                assert_eq!(text, "I found");
                assert_eq!(completion_tokens, Some(12));
                assert_eq!(calls.len(), 2);
                assert_eq!(calls[0].id, "call_1");
                assert_eq!(calls[0].name, "read_file");
                assert_eq!(calls[0].arguments, r#"{"path":"src/main.rs"}"#);
                assert_eq!(calls[1].id, "call_2");
                assert_eq!(calls[1].name, "ls");
                assert_eq!(calls[1].arguments, r#"{"path":"src"}"#);
            }
            AssistantTurn::Completed { .. } => panic!("missing tool calls"),
        }
    }
}
