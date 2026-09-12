use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use std::process::Stdio;
use std::time::Duration;

use crate::tools::{Tool, ToolError};

#[derive(Deserialize, JsonSchema)]
struct ShellCommandArgs {
    command: String,
}

pub struct ShellCommand {}

impl ShellCommand {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for ShellCommand {
    fn name(&self) -> &str {
        "shell_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command with specified arguments"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ShellCommandArgs)).unwrap()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
        let args: ShellCommandArgs =
            serde_json::from_value(args).map_err(|e| ToolError(e.to_string()))?;

        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(args.command)
            .current_dir(".")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let limit = Duration::from_millis(5000);

        let result = match tokio::time::timeout(limit, child.wait_with_output()).await {
            Ok(out) => format!("{:?}", out?), // format_output(out?),
            Err(_) => format!(
                "Comando terminato: timeout di {}s superato",
                limit.as_secs()
            ),
        };

        Ok(result)
    }
}
