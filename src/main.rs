mod agent;
mod context;
mod error;
mod message;
mod provider;
mod tools;
mod ui;

use crate::{
    agent::Agent,
    context::Conversation,
    provider::openai::OpenAiClient,
    tools::{ls::Ls, registry::ToolRegistry},
    ui::render::StdoutUi,
};
use anyhow::Result;
use clap::Parser;
use rustyline::DefaultEditor;
use std::process::exit;

#[derive(clap::Parser)]
struct Cli {
    #[arg(long, default_value = "http://localhost:8080/v1")]
    base_url: String,
    #[arg(long, default_value = "phi3")]
    model: String,
    #[arg(long)]
    system: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut tools = ToolRegistry::new();
    tools.register(Box::new(Ls::new(std::env::current_dir()?)));

    let llm = Box::new(OpenAiClient::new(cli.base_url, cli.model));
    let ui = Box::new(StdoutUi);
    let agent = Agent::new(llm, tools, ui);
    let mut conversation = Conversation::new(cli.system);

    println!("I'm crab!");
    let mut rl = DefaultEditor::new()?;

    loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(line) => match line.as_str() {
                "/clear" => conversation.reset(),
                "/quit" => exit(0),
                line => {
                    if let Err(e) = agent.run_turn(&mut conversation, line).await {
                        eprintln!("error: {e}");
                    }
                }
            },
            _ => {
                println!("Bye");
                break;
            }
        }
    }
    Ok(())
}
