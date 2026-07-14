use anyhow::Result;
use rustyline::{DefaultEditor};

#[tokio::main]
async fn main() -> Result<()> {
    println!("I'm crab!");
    let mut rl = DefaultEditor::new()?;
    loop {

        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                println!("Line: {}", line);
            },
            _ => {
                println!("Bye");
                break
             }
        }
    }
    Ok(())
}
