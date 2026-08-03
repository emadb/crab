use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use crate::tools::{Tool, ToolError};

#[derive(Deserialize, JsonSchema)]
struct LsArgs {
    #[serde(default = "default_path")]
    path: String,
}

fn default_path() -> String {
    ".".to_string()
}

pub struct Ls {
    root: PathBuf,
}

impl Ls {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

#[async_trait]
impl Tool for Ls {
    fn name(&self) -> &str {
        "list_files"
    }

    fn description(&self) -> &str {
        "List files and directories in a path. Output is a tree-like listing with a cap on the number of entries."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(LsArgs)).unwrap()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
        let args: LsArgs = serde_json::from_value(args).map_err(|e| ToolError(e.to_string()))?;

        let target = self.root.join(&args.path);
        if !target.is_dir() {
            return Err(ToolError(format!("'{}' is not a directory", args.path)));
        }

        let entries: Vec<PathBuf> = fs::read_dir(&target)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path().to_path_buf())
            .take(200)
            .collect();

        let cap = entries.len();
        let mut lines: Vec<String> = entries
            .iter()
            .map(|p| {
                let rel = p.strip_prefix(&self.root).unwrap_or(p);
                let icon = if p.is_dir() { "D" } else { " " };
                format!("{icon} {}", rel.display())
            })
            .collect();

        lines.insert(0, format!("{} entries\n---", cap));

        if cap >= 200 {
            lines.push("(truncated at 200 entries)".to_string());
        }

        Ok(lines.join("\n"))
    }
}
