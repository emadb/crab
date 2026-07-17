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
use std::{io::Write, process::exit, time::Duration};

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

const MAX_RETRIES: u32 = 3;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let llm_config = LlmConfig {
        base_url: cli.base_url,
        model: cli.model,
    };

    println!("I'm crab!");
    let mut rl = DefaultEditor::new()?;
    let mut history: Vec<Message> = vec![];
    let add_system_prompt = add_system_prompt(cli.system.clone());
    add_system_prompt(&mut history);

    let client: Box<dyn LlmClient> =
        Box::new(ChatServer::new(llm_config.base_url, llm_config.model));

    loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                send_prompt(line, &mut history, client.as_ref(), &add_system_prompt).await;
            }
            _ => {
                println!("Bye");
                break;
            }
        }
    }
    Ok(())
}

fn add_system_prompt(system: Option<String>) -> impl Fn(&mut Vec<Message>) {
    move |h| {
        if let Some(system) = &system {
            h.push(Message::system(system.clone()));
        }
    }
}

async fn send_prompt(
    line: String,
    history: &mut Vec<Message>,
    client: &dyn LlmClient,
    add_system_prompt: &impl Fn(&mut Vec<Message>),
) {
    match line.as_str() {
        "/clear" => {
            history.clear();
            add_system_prompt(history);
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

/// Apre lo stream con retry + backoff esponenziale sugli errori retryable,
/// poi lo consuma. Un errore a stream già iniziato risale senza retry.
async fn chat_turn(client: &dyn LlmClient, history: &[Message]) -> Result<String, LlmError> {
    let mut attempt = 0u32;
    let stream = loop {
        match client.send_message(history).await {
            Err(e) if e.is_retryable() && attempt < MAX_RETRIES => {
                let delay = e
                    .retry_after()
                    .unwrap_or_else(|| Duration::from_millis(500 * 2u64.pow(attempt)));
                eprintln!("[{e} — riprovo tra {}ms]", delay.as_millis());
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            other => break other?,
        }
    };
    consume_stream(stream).await
}

async fn consume_stream(mut stream: ChatStream) -> Result<String, LlmError> {
    let mut full_response = String::new();
    let mut stdout = std::io::stdout();
    while let Some(event) = stream.try_next().await? {
        match event {
            StreamEvent::TextDelta(token) => {
                print!("{token}");
                let _ = stdout.flush();
                full_response.push_str(&token);
            }
            StreamEvent::Usage { .. } => {} // step 8: alimenterà l'indicatore [ctx: N%]
            StreamEvent::Done => break,
        }
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
