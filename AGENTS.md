# AGENTS.md

This file provides guidance to agents (i.e., ADAL) when working with code in this repository.

## Project Overview

CodeAgent is a Rust-based CLI for building interactive coding agents with LLM support (OpenAI and Anthropic). It features a REPL interface, tool execution system, and persistent session management.

**Key Tech Stack:**
- Language: Rust 1.70+
- LLM SDKs: `async-openai` for OpenAI, custom Anthropic client via `reqwest`
- Tools: File ops, code search (ripgrep), command execution
- Storage: JSON sessions in `~/.codeagent/sessions/`

## Essential Commands

### Development & Testing

```bash
# Build (development mode)
cargo build

# Run in development (with provider and API key)
cargo run -- --provider openai --api-key sk-...

# Run tests
cargo test

# Build release binary
cargo build --release
# Binary location: target/release/codeagent
```

### Running the Agent

```bash
# With API key as argument
codeagent --provider openai --api-key sk-... --directory /path/to/project

# Using environment variables (recommended)
export OPENAI_API_KEY=sk-...
codeagent --provider openai

export ANTHROPIC_API_KEY=sk-ant-...
codeagent --provider anthropic

# Resume existing session
codeagent --provider openai --session <session-id>
```

### Critical Dependencies

- **Grep tools** (for `FileSearchTool` code search):
  - **ripgrep** (recommended): Fastest, supports file type filters, installed via `cargo install ripgrep`
  - **grep** (Unix fallback): Available on most Unix systems by default
  - **findstr** (Windows fallback): Available on Windows by default
  - Tool selection: Automatically tries ripgrep → grep → findstr (first available wins)
- **Web search**: Optional, supports two providers:
  - Serper API: Set `SERPER_API_KEY` env var for `WebSearchTool` (premium, requires API key from serper.dev)
  - DuckDuckGo: `WebSearchDDGTool` works out of the box (free, no API key needed)

## Architecture

### System Flow

```
User Input (REPL)
  → Session Management (stores history)
  → Provider (OpenAI/Anthropic streaming)
  → Tool Calls (detected from LLM response)
  → Tool Execution (FileSearch, EditFile, Bash)
  → Results fed back to LLM
  → Loop until completion
  → Session saved to ~/.codeagent/sessions/<id>.json
```

### Provider System (`src/provider/`)

**Trait-based abstraction** for LLM providers:
- `LLMProvider` trait defines: `chat_completion()`, `stream_completion()`
- Implementations: `OpenAIProvider` (uses `async-openai` SDK), `AnthropicProvider` (custom HTTP client)
- Both return `StreamChunk` with content + tool calls
- Tool definitions passed as JSON to LLM in Anthropic/OpenAI format

**Key types:**
- `Message`: Standard chat message (role + content)
- `ToolCall`: LLM-requested tool execution (id + name + arguments)
- `StreamChunk`: Streaming response unit (content, tool_calls, finished flag)

### Tool System (`src/tools/`)

**Registry pattern** for tool management:
1. Each tool implements `Tool` trait: `name()`, `description()`, `input_schema()`, `execute()`
2. `ToolRegistry` maintains `HashMap<String, Box<dyn Tool>>`
3. Tools registered in `src/tools/mod.rs`:
   - `ToolRegistry::new()`: Registers basic tools only (FileSearch, EditFile, Bash)
   - `ToolRegistry::new_with_api_keys()`: Adds web search tools (conditionally based on API keys)
4. LLM receives tool definitions as JSON schemas
5. Tool calls executed synchronously, results added to conversation

**Built-in tools:**
- `FileSearchTool`: glob (find files) + grep (search content via ripgrep)
- `EditFileTool`: create_file, replace_by_string, replace_by_lines, read_file
- `BashTool`: Execute shell commands in working directory
- `WebSearchTool`: Web search via Serper API (requires SERPER_API_KEY)
- `WebSearchDDGTool`: Web search via DuckDuckGo (free, no API key required)
- `URLFetchTool`: Fetch and extract content from URLs (HTML to text/markdown)

**Tool output format:**
```rust
pub struct ToolOutput {
    pub output: serde_json::Value,  // Structured data for LLM
    pub observation: String,          // Human-readable summary
    pub display: Option<String>,      // Optional detailed display
    pub status: String,               // "success" | "error"
}
```

### Session Management (`src/session/mod.rs`)

**Persistent conversation storage:**
- Sessions stored as JSON in `~/.codeagent/sessions/<uuid>.json`
- Each session contains: metadata, full message history, tool calls, tool results
- Auto-saves after each user interaction and on exit
- Resume via `--session <id>` flag

**Data structures:**
- `SessionInfo`: Metadata (id, title, directory, timestamps, message count)
- `MessagePart`: Single conversation turn with role, content, tool_calls, tool_results
- `ToolResult`: Tool execution outcome (tool_call_id, output, observation, status)

**Conversation reconstruction:**
- `get_conversation_history()` flattens MessageParts into simple `Message` list
- Tool results injected as user messages: "Tool result: {observation}"
- This simplified view fed to LLM for context

### Entry Point (`src/main.rs`)

**REPL loop logic:**
1. Parse CLI args (provider, API key, working dir, session)
2. Initialize provider + tool registry
3. Load/create session
4. Loop:
   - Get user input
   - Add to session history
   - Stream LLM response (with tool definitions)
   - Collect tool calls from response
   - Execute tools via registry
   - Add tool results to session + conversation
   - If tool calls present, loop again with results as context
   - If no tool calls, wait for next user input
5. Auto-save session on exit

**Special commands:**
- `save`: Manually save session
- `exit`: Save and quit

## Key Implementation Details

### Tool Call Loop (Agent Pattern)

The agent continues calling tools until the LLM returns a response without tool calls:

```rust
loop {
    // Stream LLM response
    let response = provider.stream_completion(messages, tools).await?;
    
    // Add assistant message to history
    session.add_assistant_message(content, tool_calls);
    
    // If no tool calls, we're done
    if tool_calls.is_empty() { break; }
    
    // Execute tools and add results to messages
    for tool_call in tool_calls {
        let result = tool_registry.execute(&tool_call.name, tool_call.arguments)?;
        session.add_tool_result(result);
        messages.push(Message { role: "user", content: result.observation });
    }
}
```

### Streaming Response Handling

Both providers use `tokio::sync::mpsc::Receiver<StreamChunk>`:
- Main thread spawns async task for API calls
- Chunks sent via channel as they arrive
- REPL prints content immediately for real-time display
- Tool calls accumulated until stream finishes

### System Prompt

Hardcoded in `main.rs`:
```
"You are a helpful coding assistant. You have access to tools for file operations, 
code search, and command execution. Use them to help the user with their coding tasks."
```

Prepended to conversation history before every LLM call.

## Gotchas & Constraints

1. **Ripgrep dependency**: FileSearchTool will panic if `rg` not in PATH
2. **No test coverage**: `cargo test` exists but no actual test files present
3. **Single working directory**: Set at startup via `--directory`, not changeable mid-session
4. **Tool execution is synchronous**: No parallel tool calls (executes sequentially)
5. **Session format is append-only**: No editing past messages, full history retained
6. **API keys**: Must be provided via CLI arg or env var, not stored in sessions
7. **Provider models**: Defaults used unless `--model` specified:
   - OpenAI: Model set in provider initialization (check `src/provider/openai.rs`)
   - Anthropic: Model set in provider initialization (check `src/provider/anthropic.rs`)

## File Organization

```
src/
├── main.rs              # CLI + REPL loop
├── provider/
│   ├── mod.rs           # LLMProvider trait + types
│   ├── openai.rs        # OpenAI implementation
│   └── anthropic.rs     # Anthropic implementation
├── session/
│   └── mod.rs           # Session storage & history
└── tools/
    ├── mod.rs           # Tool trait + registry
    ├── file_search.rs   # Glob + grep tools
    ├── edit_file.rs     # File editing tools
    ├── bash.rs          # Command execution
    ├── web_search.rs    # Serper API web search
    ├── web_search_ddg.rs # DuckDuckGo web search (free)
    └── url_fetch.rs     # URL content fetching
```

## Development Workflow

1. **Adding a new tool:**
   - Implement `Tool` trait in `src/tools/your_tool.rs`
   - Add module declaration in `src/tools/mod.rs`
   - Register in `ToolRegistry::new()` in `src/tools/mod.rs`
   - Tool automatically available to LLM (schema extracted from trait methods)

2. **Adding a new provider:**
   - Implement `LLMProvider` trait in `src/provider/your_provider.rs`
   - Add module declaration in `src/provider/mod.rs`
   - Add match arm in `main.rs` provider initialization
   - Handle API key env var in args parsing

3. **Modifying session format:**
   - Update types in `src/session/mod.rs`
   - Update serialization/deserialization
   - **No migration needed** - old sessions incompatible, user creates new session
