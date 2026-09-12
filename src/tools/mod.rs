use async_trait::async_trait;

pub mod edit_file;
pub mod grep;
pub mod ls;
pub mod read_file;
pub mod registry;
pub mod shell_command;
pub mod write_file;

#[derive(thiserror::Error, Debug)]
#[error("tool error: {0}")]
pub struct ToolError(pub String);

impl From<std::io::Error> for ToolError {
    fn from(value: std::io::Error) -> Self {
        ToolError(value.to_string())
    }
}

pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;
    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError>;
}
