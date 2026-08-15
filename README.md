# Crab

Crab is a coding agent designed to assist with software development tasks. It can interact with the file system, execute shell commands, and communicate with Large Language Models (LLMs) to perform complex coding operations.

## Features

- **Tool Integration**: Crab supports various tools including `ls`, `read_file`, `grep`, `write_file`, and `shell_command`.\n- **Conversational Interface**: It provides an interactive CLI where users can chat with the agent and see its actions in real-time.\n- **LLM Agnostic**: Currently supports OpenAI-compatible APIs, allowing it to work with models like GPT-4, Claude, or local models served via Ollama/LM Studio.\n- **Context Awareness**: Maintains a conversation history to provide consistent and relevant assistance.

## Project Structure

The project is organized as follows:

- `src/main.rs`: The entry point of the application. It handles CLI argument parsing, tool registration, and the main interaction loop.\n- `src/agent.rs`: Contains the core logic for the agent, including the thought process and tool execution loop.\n- `src/context.rs`: Manages the conversation history and system prompts.\n- `src/message.rs`: Defines the data structures for messages exchanged between the user, the agent, and the LLM.\n- `src/provider/`: Contains the clients for interacting with different LLM providers (e.g., OpenAI).\n- `src/tools/`: Contains the definitions and implementations of all tools available to the agent (ls, grep, read_file, write_file, shell_command).\n- `src/ui/`: Handles the rendering of the interface to the terminal.\n- `src/error.rs`: Defines custom error types for the project.

## How to Run

### Prerequisites

- Rust and Cargo installed on your system.\n- An OpenAI-compatible API key (or a local server like Ollama).

### Installation

1. Clone the repository:
```bash\n   git clone <repository_url>\n   cd crab\n   ```

2. Install dependencies:
```bash
cargo build
```

### Running the Application

You can run the agent using the following command:

```bash
cargo run -- --base-url <YOUR_BASE_URL> --model <YOUR_MODEL_NAME> --system \"<YOUR_SYSTEM_PROMPT>\"
```

**Arguments:**
- `--base-url`: The URL of the LLM API (default: `http://localhost:8080/v1`).
- `--model`: The name of the model to use (default: `phi3`).
- `--system`: (Optional) A custom system prompt for the agent.

### Interactive Commands

Inside the Crab CLI, you can use the following commands:
- `/clear`: Clears the current conversation context.
- `/quit`: Exits the application.\n