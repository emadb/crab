mod agent;
mod context;
mod error;
mod conversation_entry;
mod message;
mod provider;
mod tools;
mod ui;

use crate::{
    agent::Agent,
    context::Conversation,
    provider::openai::OpenAiClient,
    tools::{
        edit_file::Edit, grep::Grep, ls::Ls, read_file::ReadFile, registry::ToolRegistry,
        shell_command::ShellCommand, write_file::WriteFile,
    },
    ui::render::StdoutUi,
};
use anyhow::Result;
use clap::Parser;
use rustyline::DefaultEditor;
use std::process::exit;

#[derive(clap::Parser)]
struct Cli {
    #[arg(long, default_value = "CRAB_API_KEY")]
    api_key_env: String,
    #[arg(long, default_value = "http://localhost:8080/v1")]
    base_url: String,
    #[arg(long, default_value = "phi3")]
    model: String,
    #[arg(
        long,
        default_value = "You are a coding agent specialized in writing clean and simple code. You have three tools: `ls` to list the content of a specific folder, `read_file` to read the content of a file and `grep` to search a pattern inside a file. Explore the content of the folder and think before sending a response to the user. If you need more details about a particular topic, ask the user, don't invent answers or take a decision without having all the informations"
    )]
    system: Option<String>,
    #[arg(long, default_value = "32768")]
    context_size: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut tools = ToolRegistry::new();
    tools.register(Box::new(Ls::new(std::env::current_dir()?)));
    tools.register(Box::new(ReadFile::new(std::env::current_dir()?)));
    tools.register(Box::new(Grep::new()));
    tools.register(Box::new(WriteFile::new(std::env::current_dir()?)));
    tools.register(Box::new(ShellCommand::new()));
    tools.register(Box::new(Edit::new()));

    let api_key = std::env::var(&cli.api_key_env).ok();

    let llm = Box::new(OpenAiClient::new(cli.base_url, cli.model, api_key));
    let ui = Box::new(StdoutUi);
    let agent = Agent::new(llm, tools, ui);
    let mut conversation = Conversation::new(cli.system, cli.context_size);

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
                    // for c in &conversation.messages {
                    //     println!("- {:?}", c)
                    // }
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
