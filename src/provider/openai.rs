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
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
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
            .json(&ChatRequest {
                model: self.model.clone(),
                messages: req.messages.iter().map(|m| build_message(&m.message)).collect(),
                stream: true,
                tools: tools_json(req.tools),
                stream_options: Some(StreamOptions { include_usage: true }),
            });

        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }

        let response = request
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let mut events = Box::pin(sse_events(response.bytes_stream()));
        let mut text = String::new();
        let mut calls: Vec<ToolCall> = Vec::new();
        let mut usage: UsageInfo =  UsageInfo { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };

        while let Some(data) = events.try_next().await? {
            let parsed: ChunkResponse = serde_json::from_str(&data)?;

            if let Some(u) = parsed.usage {
                usage = u;
            }

            let Some(choice) = parsed.choices.into_iter().next() else {
                continue;
            };

            if let Some(token) = choice.delta.content {
                text.push_str(&token);
                on_delta(Delta::Text(token));
            }

            for tc in choice.delta.tool_calls.unwrap_or_default() {
                if tc.index >= calls.len() {
                    calls.push(ToolCall {
                        id: String::new(),
                        name: String::new(),
                        arguments: String::new(),
                    });
                }

                let call = &mut calls[tc.index];
                if let Some(name) = tc.function.as_ref().and_then(|f| f.name.clone()) {
                    if call.name.is_empty() {
                        on_delta(Delta::ToolCallStarted { name: name.clone() });
                    }
                    call.name = name;
                }
                if let Some(id) = tc.id {
                    call.id = id;
                }
                if let Some(function) = tc.function {
                    call.arguments.push_str(&function.arguments);
                }
            }
        }

        println!("PROMPT: {}", usage.prompt_tokens);
        println!("COMPLE: {}", usage.completion_tokens);

        Ok(if calls.is_empty() {
            AssistantTurn::Completed {
                text,
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens
            }
        } else {
            AssistantTurn::ToolCalls {
                text, calls,
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens
            }
        })
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
