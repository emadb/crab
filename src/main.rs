mod api_client;
mod message;

use std::process::exit;

use crate::{api_client::send_message, message::Message};
use anyhow::{Error, Result};
use rustyline::DefaultEditor;

#[tokio::main]
async fn main() -> Result<()> {
    println!("I'm crab!");
    let mut rl = DefaultEditor::new()?;
    let mut history: Vec<Message> = vec![];
    loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                manage_line(line, &mut history).await;
            }
            _ => {
                println!("Bye");
                break;
            }
        }
    }
    Ok(())
}

async fn manage_line(line: String, history: &mut Vec<Message>) {
    match line.as_str() {
        "/clear" => {
            history.clear();
        }
        "/quit" => {
            exit(0);
        }
        line => {
            history.push(message::Message::user(line));
            let res = send_message(history).await;
            manage_response(res, history);
        }
    }
}

fn manage_response(content: Result<String, Error>, history: &mut Vec<Message>) {
    match content {
        Ok(line) => {
            history.push(message::Message::assistant(line.clone()));
            println!("{}\n", line)
        }
        Err(e) => println!("ERROR: {:?}", e),
    }
}
