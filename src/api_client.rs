use crate::message::Message;
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

pub async fn send_message(history: &Vec<Message>) -> Result<String> {
    let client = reqwest::Client::new();
    let base_url = "http://localhost:8080/v1";
    let url = format!("{}/chat/completions", base_url);

    let response = client
        .post(&url)
        // .bearer_auth(api_key)
        .json(&ChatRequest {
            model: "gemma4".to_string(),
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
