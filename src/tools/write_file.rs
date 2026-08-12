use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use crate::tools::{Tool, ToolError};

#[derive(Deserialize, JsonSchema)]
struct WriteFileArgs {
    file: String,
    content: String,
}

pub struct WriteFile {
    root: PathBuf,
}

impl WriteFile {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

#[async_trait]
impl Tool for WriteFile {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a new file or an existing file. The content will be completely write inside the file. Any existing content will be overwritten."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(WriteFileArgs)).unwrap()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
        let args: WriteFileArgs = serde_json::from_value(args).map_err(|e| ToolError(e.to_string()))?;

        let target = self.root.join(&args.file);
        if target.is_dir() {
            return Err(ToolError(format!("'{}' is a directory", args.file)));
        }

        let file = open_or_create_file(target, args.file);

        if file.is_err() {
            return Err(ToolError(format!("Error while opening or creting the file")))
        }

        let mut file = file.unwrap();
        let _ = file.write_all(args.content.as_bytes());
        let _ = file.flush();


        Ok(args.content)
    }
}

fn open_or_create_file(target: PathBuf, file: String) -> Result<File, std::io::Error> {
    if !target.exists() {
        File::create(file)
    } else {
        File::open(file)
    }
}