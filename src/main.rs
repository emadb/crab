mod message;

use anyhow::Result;
use rustyline::{DefaultEditor};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() -> Result<()> {
    println!("I'm crab!");
    let mut rl = DefaultEditor::new()?;
    loop {

        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                let res = send_message(line).await;
                println!("{}\n", res.unwrap());
            },
            _ => {
                println!("Bye");
                break
             }
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}


async fn send_message(line: String) -> Result<String> {
    let client = reqwest::Client::new();
    let base_url = "http://localhost:8080/v1";
    let url = format!("{}/chat/completions", base_url);
    let msg = Message{ role: "user".to_string(), content: line};
    let response = client
            .post(&url)
            // .bearer_auth(api_key)
            .json(&ChatRequest { model: "gemma4".to_string(), messages: vec![msg] })
            .send()
            .await?;

    // let status = response.status();
    let body = response
        .text()
        .await;

    match body {
        Ok(content) => {
            let parsed: ChatResponse = serde_json::from_str(&content)?;
            Ok(parsed.choices[0].message.content.clone())
        },
        Err(e) => Err(e.into())
    }
}