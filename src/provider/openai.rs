use crate::{
    message::{Message, ToolCall},
    provider::{LlmClient, LlmError, TurnRequest},
    tools::ToolSpec,
};
use anyhow::{Context, Result};
use reqwest_sse::EventSource;
use serde::{Deserialize, Serialize};

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

#[async_trait::async_trait(?Send)]
impl LlmClient for OpenAiClient {
    async fn send(
        &self,
        req: TurnRequest<'_>,
        on_delta: &mut (dyn FnMut(ChunkResponse) + Send),
    ) -> anyhow::Result<ConversationEntry> {
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
            response.text().await?;
            return Err(anyhow::Error::msg("Llm Error"));
        }


        let mut events = response.events().await.unwrap();
        let mut accumulator = ChunkAccumulator::default();

        while let Some(evt) = events.next().await {
            let data = evt.unwrap().data;
            if data == "[DONE]" {
                break;
            }
            let chunk: ChunkResponse = serde_json::from_str(&data).unwrap();
            println!("> {:?}", &chunk);
            // on_delta(chunk);
            accumulator.push(chunk);
        }

        accumulator.finish()

        // TODO
        // Ok(AssistantTurn:: Completed{text: String::from("ciao"), completion_tokens: None})

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

#[derive(Debug, Deserialize)]
#[serde(try_from = "RawChunk")]
pub enum ChunkResponse {
    // Include il chunk iniziale con role=assistant e content=null.
    Message {
        index: u32,
        delta: Delta,
    },

    ToolCalls {
        index: u32,
        delta: Delta,
    },

    Finished {
        index: u32,
        reason: FinishReason,
        // Conserviamo eventuali dati presenti anche nell'ultimo delta.
        delta: Delta,
    },

    Usage {
        usage: Usage,
    },
}


#[derive(Debug, serde::Deserialize)]
pub struct Choice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Deserialize)]
pub struct Delta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub refusal: Option<String>,
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    pub id: Option<String>,

    #[serde(rename = "type")]
    pub kind: Option<String>,

    pub function: Option<FunctionDelta>,
}

#[derive(Debug, Deserialize)]
pub struct FunctionDelta {
    pub name: Option<String>,

    // Frammento di JSON: va concatenato, non parsato subito.
    pub arguments: Option<String>,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    FunctionCall,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub completion_tokens: u64,
    pub prompt_tokens: u64,
    pub total_tokens: u64,
    pub prompt_tokens_details: Option<PromptTokensDetails>,
    pub completion_tokens_details: Option<CompletionTokensDetails>,
}

#[derive(Debug, Deserialize)]
pub struct PromptTokensDetails {
    pub cached_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CompletionTokensDetails {
    pub reasoning_tokens: Option<u64>,
}


#[derive(Debug, Deserialize)]
struct RawChunk {
    choices: Vec<RawChoice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct RawChoice {
    index: u32,
    delta: Delta,
    finish_reason: Option<FinishReason>,
}

impl TryFrom<RawChunk> for ChunkResponse {
    type Error = String;

    fn try_from(raw: RawChunk) -> Result<Self, Self::Error> {
        let RawChunk { mut choices, usage } = raw;

        if choices.is_empty() {
            return usage
                .map(|usage| Self::Usage { usage })
                .ok_or_else(|| "Chunk senza choices e senza usage".to_owned());
        }

        if usage.is_some() {
            return Err("Atteso usage in un chunk separato".to_owned());
        }

        let choice = choices.pop().ok_or("Choice mancante")?;

        let RawChoice {
            index,
            delta,
            finish_reason,
        } = choice;

        if let Some(reason) = finish_reason {
            return Ok(Self::Finished {
                index,
                reason,
                delta,
            });
        }

        if delta
            .tool_calls
            .as_ref()
            .is_some_and(|calls| !calls.is_empty())
        {
            return Ok(Self::ToolCalls { index, delta });
        }

        Ok(Self::Message { index, delta })
    }
}

// Conversation

#[derive(Debug)]
pub enum ConversationEntry {
    System {
        text: String,
    },
    UserInput {
        text: String,
    },
    Model {
        text: Option<String>,
        refusal: Option<String>,
        tool_calls: Vec<ToolCall>,
        finish_reason: FinishReason,
        usage: Option<Usage>,
    },
    ToolOutput {
        tool_call_id: String,
        output: String,
    },
}

// #[derive(Debug)]
// pub struct ToolCall {
//     pub id: String,
//     pub name: String,
//     // JSON completo degli argomenti, conservato come stringa.
//     pub arguments: String,
// }

#[derive(Default)]
struct PendingToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Default)]
pub struct ChunkAccumulator {
    text: Option<String>,
    refusal: Option<String>,
    tools: std::collections::BTreeMap<u32, PendingToolCall>,
    finish_reason: Option<FinishReason>,
    usage: Option<Usage>,
}

impl ChunkAccumulator {
    pub fn push(&mut self, chunk: ChunkResponse) {
        let delta = match chunk {
            ChunkResponse::Message { delta, .. }
            | ChunkResponse::ToolCalls { delta, .. } => delta,

            ChunkResponse::Finished { reason, delta, .. } => {
                self.finish_reason = Some(reason);
                delta
            }

            ChunkResponse::Usage { usage } => {
                self.usage = Some(usage);
                return;
            }
        };

        append(&mut self.text, delta.content);
        append(&mut self.refusal, delta.refusal);

        for call in delta.tool_calls.unwrap_or_default() {
            let tool = self.tools.entry(call.index).or_default();

            if let Some(id) = call.id {
                tool.id = Some(id);
            }

            if let Some(function) = call.function {
                if let Some(name) = function.name {
                    tool.name = Some(name);
                }

                if let Some(arguments) = function.arguments {
                    tool.arguments.push_str(&arguments);
                }
            }
        }
    }

    pub fn finish(self) -> Result<ConversationEntry> {
        let finish_reason = self
            .finish_reason
            .context("Risposta incompleta: manca finish_reason")?;

        let tool_calls = self.tools.into_values()
            .map(|tool| {
                Ok(ToolCall {
                    id: tool.id.context("Tool senza ID")?,
                    name: tool.name.context("Tool senza nome")?,
                    arguments: tool.arguments,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(ConversationEntry::Model {
            text: self.text,
            refusal: self.refusal,
            tool_calls,
            finish_reason,
            usage: self.usage,
        })
    }
}

fn append(target: &mut Option<String>, fragment: Option<String>) {
    if let Some(fragment) = fragment {
        target.get_or_insert_with(String::new).push_str(&fragment);
    }
}