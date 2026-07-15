mod api_client;
mod message;

use std::process::exit;
use crate::{api_client::send_message, message::Message};
use anyhow::{Error, Result};
use clap::Parser;
use rustyline::DefaultEditor;

#[derive(clap::Parser)]
struct Cli {
    #[arg(long, default_value = "http://localhost:8080/v1")]
    base_url: String,
    #[arg(long, default_value = "gemma4")]
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
    let mut history: Vec<Message> = vec![];
    let add_system_prompt = add_system_prompt(cli.system.clone());
    add_system_prompt(&mut history);
    loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                manage_line(line, &mut history, &llm_config, &add_system_prompt).await;
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

async fn manage_line(
    line: String,
    history: &mut Vec<Message>,
    llm_config: &LlmConfig,
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
            let res = send_message(history, llm_config).await;
            manage_response(res, history);
        }
    }
}

fn manage_response(content: Result<String, Error>, history: &mut Vec<Message>) {
    match content {
        Ok(line) => {
            history.push(Message::assistant(line));
            // i token sono già stati stampati in streaming, chiudiamo solo la riga
            println!("\n")
        }
        Err(e) => println!("ERROR: {:?}", e),
    }
}
