use crate::ui::{AgentEvent, Ui};
use std::io::Write;

pub struct StdoutUi;

impl Ui for StdoutUi {
    fn emit(&self, event: AgentEvent) {
        match event {
            AgentEvent::TextDelta(text) => {
                print!("{text}");
                let _ = std::io::stdout().flush();
            }
            AgentEvent::ToolStarted { name } => println!("\n[tool] {name}..."),
            AgentEvent::ToolExecutionStarted { name, arguments } => {
                println!("[tool] {name} {arguments}");
            }
            AgentEvent::ToolFinished { name, result } => {
                if result.is_error {
                    println!("[tool] {name} failed: {}", result.content);
                } else {
                    println!("[tool] {name} done");
                }
            }
            AgentEvent::TurnEnded => println!(),
            AgentEvent::Error(err) => eprintln!("error: {err}"),
        }
    }
    fn ask_permission(&self) -> bool {
        let term = console::Term::stdout();
        println!("Proceed with tool execution? [y/N]");

        match term.read_char() {
            Ok(c) => c == 'y',
            _ => false,
        }
    }
}
