use crate::{
    message::{AssistantTurn, Message}, provider::{Delta, LlmClient, LlmError, TurnRequest, turn_accumulator::TurnAccumulator}, tools::ToolSpec,
};
use reqwest_sse::EventSource;
use serde::{Serialize};

use tokio_stream::StreamExt;

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

#[derive(serde::Deserialize, Debug)]
pub struct UsageInfo {
    // pub prompt_tokens: usize,
    // pub total_tokens: usize,
    pub completion_tokens: usize,
}

fn decode_chunk(data: &str) -> Result<super::turn_accumulator::ChunkResponse, LlmError> {
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

#[async_trait::async_trait(?Send)]
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
            let body = response.text().await?;
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let mut turn = TurnAccumulator::default();
        let mut events = response.events().await.unwrap();
        while let Some(evt) = events.next().await {
            let data = evt.unwrap().data;
            if data == "[DONE]" {
                break;
            }
            for delta in turn.apply(decode_chunk(&data)?) {
                on_delta(delta);
            }
        }

        turn.finish()
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