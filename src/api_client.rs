use crate::{LlmConfig, message::Message};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

pub async fn send_message(history: &[Message], llm_config: &LlmConfig) -> Result<String> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", llm_config.base_url);

    let response = client
        .post(&url)
        // .bearer_auth(api_key)
        .json(&ChatRequest {
            model: llm_config.model.to_string(),
            messages: history.to_vec(),
        })
        .send()
        .await?;

    let status = response.status();
    let content = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!("server returned {}: {}", status, content));
    }

    let parsed: ChatResponse = serde_json::from_str(&content)?;
    let res = parsed
        .choices
        .first()
        .map_or("no content".to_string(), |c| c.message.content.clone());
    Ok(res)
}
