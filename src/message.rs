#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub enum AssistantTurn {
    Completed { text: String, completion_tokens: usize },
    ToolCalls { text: String, calls: Vec<ToolCall>,  completion_tokens: usize },
}

#[derive(Debug, Clone)]
pub enum Message {
    System(String),
    User(String),
    Assistant {
        text: String,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        call_id: String,
        result: ToolResult,
    },
}
