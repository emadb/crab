use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;

use crate::tools::{Tool, ToolError};

#[derive(Deserialize, JsonSchema)]
struct EditArgs {
    file: String,
    old_text: String,
    new_text: String,
}


pub struct Edit {
}

impl Edit {
    pub fn new() -> Self {
        Self { }
    }
}

#[async_trait]
impl Tool for Edit {
    fn name(&self) -> &str {
        "edit_files"
    }

    fn description(&self) -> &str {
        "Replaces an exact snippet of text in a file with a new snippet. `old_str` must be unique in the file."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(EditArgs)).unwrap()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
        let args: EditArgs = serde_json::from_value(args).map_err(|e| ToolError(e.to_string()))?;


        let content: std::io::Result<String> = fs::read_to_string(&args.file);
        if content.is_err() {
            return Err(ToolError(format!("Error while reading {}", args.file)));
        }

        let content = content.unwrap();

        let occurrences = content.matches(&args.old_text).count();

        match occurrences {
            0 => Err(ToolError(
                "Error: `old_str` not found in target file. Ensure exact indentation and character matching.".into()
            )),
            1 => {
                // 2. Perform exact single replacement
                let updated = content.replacen(&args.old_text, &args.new_text, 1);
                tokio::fs::write(&args.file, updated).await?;
                Ok(format!("Successfully updated {}", args.file))
            }
            n => Err(ToolError(format!(
                "Error: `old_str` matched {} times. Include additional surrounding lines to make the match unique.",
                n
            ))),
        }
    }
}
