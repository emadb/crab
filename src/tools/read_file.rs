use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use crate::tools::{Tool, ToolError};

#[derive(Deserialize, JsonSchema)]
struct ReadFileArgs {
    #[serde()]
    file_name: String,

}

pub struct ReadFile {
    dir: PathBuf,
}

impl ReadFile {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the content of a file. Output is list of numbered lines capped to 2000."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ReadFileArgs)).unwrap()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
        let args: ReadFileArgs = serde_json::from_value(args).map_err(|e| ToolError(e.to_string()))?;

        let file_name = self.dir.join(&args.file_name);
        if file_name.is_dir() {
            return Err(ToolError(format!("'{}' is a directory", file_name.display())));
        }

        let content: std::io::Result<String> = fs::read_to_string(&file_name);
        if content.is_err() {
            return Err(ToolError(format!("Error while reading {}", file_name.display())));
        }

        let lines = content.unwrap();

        let mut n = 0;
        let mut string_line: Vec<String> = lines
            .lines()
            .take(2000)
            .map(|l| {
                n += 1;
                format!("{:>5}\t{}", n, l)
            }).collect();

        if lines.lines().count() > 2000 {
            string_line.push(format!("[...more {} lines...]", lines.lines().count()));
        }

        Ok(string_line.join("\n"))
    }
}
