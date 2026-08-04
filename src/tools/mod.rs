use async_trait::async_trait;

pub mod ls;
pub mod read_file;
pub mod grep;
pub mod registry;

#[derive(thiserror::Error, Debug)]
#[error("tool error: {0}")]
pub struct ToolError(pub String);

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
