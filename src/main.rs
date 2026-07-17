mod api_client;
mod message;

use crate::{
    api_client::{ChatServer, ChatStream, LlmClient, LlmError, StreamEvent},
    message::Message,
};
use anyhow::Result;
use clap::Parser;
use futures_util::TryStreamExt;
use rustyline::DefaultEditor;
use std::{io::Write, process::exit};

#[derive(clap::Parser)]
struct Cli {
    #[arg(long, default_value = "http://localhost:11434/v1")]
    base_url: String,
    #[arg(long, default_value = "phi3")]
    model: String,
    #[arg(long)]
    system: Option<String>,
}

struct LlmConfig {
    base_url: String,
    model: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let llm_config = LlmConfig {
        base_url: cli.base_url,
        model: cli.model,
    };

    println!("I'm crab!");
    let mut rl = DefaultEditor::new()?;
    let mut history = initial_history(&cli.system);

    let client: Box<dyn LlmClient> =
        Box::new(ChatServer::new(llm_config.base_url, llm_config.model));

    loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                send_prompt(line, &mut history, client.as_ref(), &cli.system).await;
            }
            _ => {
                println!("Bye");
                break;
            }
        }
    }
    Ok(())
}

fn initial_history(system: &Option<String>) -> Vec<Message> {
    match system {
        Some(s) => vec![Message::system(s.clone())],
        None => vec![],
    }
}

async fn send_prompt(
    line: String,
    history: &mut Vec<Message>,
    client: &dyn LlmClient,
    system: &Option<String>,
) {
    match line.as_str() {
        "/clear" => {
            *history = initial_history(system);
        }
        "/quit" => {
            exit(0);
        }
        line => {
            history.push(Message::user(line));
            let res = chat_turn(client, history).await;
            print_response(res, history);
        }
    }
}

/// Apre lo stream e lo consuma; lo stream finisce da solo al `[DONE]`.
async fn chat_turn(client: &dyn LlmClient, history: &[Message]) -> Result<String, LlmError> {
    let stream = client.send_message(history).await?;
    consume_stream(stream).await
}

async fn consume_stream(mut stream: ChatStream) -> Result<String, LlmError> {
    let mut full_response = String::new();
    let mut stdout = std::io::stdout();
    while let Some(event) = stream.try_next().await? {
        let StreamEvent::TextDelta(token) = event;
        print!("{token}");
        let _ = stdout.flush();
        full_response.push_str(&token);
    }
    Ok(full_response)
}

fn print_response(content: Result<String, LlmError>, history: &mut Vec<Message>) {
    match content {
        Ok(line) => {
            history.push(Message::assistant(line));
            // i token sono già stati stampati in streaming, chiudiamo solo la riga
            println!("\n")
        }
        Err(e) => println!("ERROR: {e}"),
    }
}
