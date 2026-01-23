# CodeAgent

A command-line interactive coding agent built in Rust with support for OpenAI and Anthropic LLMs.

## Features

- **Multi-Provider Support**: Works with both OpenAI (GPT-4o) and Anthropic (Claude) APIs
- **Interactive REPL**: Command-line interface for natural conversation with the AI
- **Tool System**: Built-in tools for file operations, code search, and command execution
- **Session Management**: Persistent conversation history with save/resume capability
- **Streaming Responses**: Real-time display of AI responses as they're generated

## Tools

### File Search Tool
- **glob**: Find files by pattern (e.g., `**/*.rs`)
- **grep**: Search file contents using ripgrep

### Edit File Tool
- **create_file**: Create new files
- **replace_by_string**: Replace unique text in files
- **replace_by_lines**: Replace line ranges in files
- **read_file**: Read file contents with optional line ranges

### Bash Tool
- Execute shell commands in the working directory

## Installation

### Prerequisites

- Rust toolchain (1.70+)
- [ripgrep](https://github.com/BurntSushi/ripgrep) installed and in PATH

### Build from Source

```bash
cd codeagent
cargo build --release
```

The binary will be in `target/release/codeagent`.

## Usage

### Basic Usage

```bash
# With OpenAI
codeagent --provider openai --api-key sk-... --directory /path/to/project

# With Anthropic
codeagent --provider anthropic --api-key sk-ant-... --directory /path/to/project

# Using environment variables
export OPENAI_API_KEY=sk-...
codeagent --provider openai

export ANTHROPIC_API_KEY=sk-ant-...
codeagent --provider anthropic
```

### Command-Line Options

```
Options:
  -p, --provider <PROVIDER>    Provider to use: openai or anthropic
  -a, --api-key <API_KEY>      API key (or set OPENAI_API_KEY/ANTHROPIC_API_KEY env var)
  -m, --model <MODEL>          Model to use (optional, uses provider default)
  -d, --directory <DIRECTORY>  Working directory [default: .]
  -s, --session <SESSION>      Session ID to resume
  -h, --help                   Print help
```

### Interactive Commands

Once in the REPL:

- Type your questions/requests naturally
- Type `save` to save the current session
- Type `exit` to quit (automatically saves)

## Examples

### Create a new file
```
You: create a hello world program in main.rs
```

### Search code
```
You: find all TODO comments in Rust files
```

### Execute commands
```
You: run cargo test
```

### Edit files
```
You: replace the println! statement in main.rs with a proper greeting function
```

## Session Storage

Sessions are stored in `~/.codeagent/sessions/` as JSON files. Each session contains:
- Conversation history
- Tool calls and results
- Metadata (title, timestamps, message count)

## Architecture

```
codeagent/
├── src/
│   ├── main.rs           # CLI entry point and REPL
│   ├── provider/         # LLM provider implementations
│   │   ├── mod.rs        # Provider trait
│   │   ├── openai.rs     # OpenAI integration
│   │   └── anthropic.rs  # Anthropic integration
│   ├── session/          # Session management
│   │   └── mod.rs        # Session storage and history
│   └── tools/            # Tool implementations
│       ├── mod.rs        # Tool registry
│       ├── file_search.rs
│       ├── edit_file.rs
│       └── bash.rs
└── Cargo.toml
```

## Development

### Run in development mode

```bash
cargo run -- --provider openai --api-key sk-...
```

### Run tests

```bash
cargo test
```
