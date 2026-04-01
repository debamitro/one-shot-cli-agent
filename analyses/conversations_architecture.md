# How Conversations Are Saved

Conversations in this CodeAgent project are saved through a session-based system. Here's how it works:

## Storage Location

- **Storage path**: `~/.codeagent/sessions/` (src/main.rs:158-161)
- **File format**: JSON files named `{session_id}.json` (src/session/mod.rs:74)

## Session Data Structure

Each session contains two main components:

### 1. SessionInfo (src/session/mod.rs:10-20)

```rust
pub struct SessionInfo {
    pub id: String,                    // Unique UUID identifier
    pub title: String,                  // Session title
    pub directory: String,              // Working directory
    pub created_at: DateTime<Utc>,      // Creation timestamp
    pub updated_at: DateTime<Utc>,      // Last update timestamp
    pub message_count: usize,           // Number of messages
    pub system_prompt: Option<String>,  // Optional system prompt
    pub persona: Option<String>,        // Optional persona name
}
```

### 2. Messages (src/session/mod.rs:23-30)

Each message contains:
- `id`: Unique message identifier (UUID)
- `role`: Message role ("user" or "assistant")
- `content`: Optional text content
- `tool_calls`: List of tool calls made by the assistant
- `tool_results`: List of results from executed tools
- `timestamp`: When the message was created

## Save Operations

### 1. Automatic Save (Interactive Mode)

In interactive REPL mode, sessions are saved automatically:
- **After each interaction** in the REPL loop (src/main.rs:560)
- **When user types `exit`** (src/main.rs:303)
- **When user types `save`** (src/main.rs:308)

### 2. Manual Save (Non-interactive Mode)

In non-interactive mode (using `--input` flag):
- **Only saves if `--save` flag is provided** (src/main.rs:766-768)
- Without the flag, the session is transient and not persisted

## The Save Function

Location: src/session/mod.rs:85-91

```rust
pub fn save(&self) -> Result<()> {
    std::fs::create_dir_all(&self.storage_path)?;
    let session_file = self.storage_path.join(format!("{}.json", self.info.id));
    let data = serde_json::to_string_pretty(&(&self.info, &self.messages))?;
    std::fs::write(session_file, data)?;
    Ok(())
}
```

The save function:
1. Creates the storage directory if it doesn't exist
2. Creates a filename using the session ID
3. Serializes session info and messages to pretty JSON
4. Writes to disk

## Loading Sessions

To resume a previous session:

```bash
codeagent --session <session_id>
```

The load function (src/session/mod.rs:73-83):
1. Reads the JSON file from the storage path
2. Deserializes the SessionInfo and Messages
3. Returns a Session object

## Export Feature

Sessions can also be exported to Markdown format for human-readable documentation:

### Usage in Interactive Mode

Type: `export [filename]`

- If no filename provided, auto-generates: `{title}_{id_prefix}.md`
- If filename provided, can be relative to session directory or absolute path

### Export Content (src/session/mod.rs:216-320)

The markdown export includes:
- Session metadata header (ID, directory, timestamps, message count)
- Full conversation history
- User messages with content
- Assistant messages with content
- Tool calls with:
  - Tool name and ID
  - Arguments (formatted as JSON)
  - Results and status

## Message Flow During Session

### Adding Messages

1. **User Message** (src/session/mod.rs:93-109):
   - Creates a new message with "user" role
   - Generates unique UUID
   - Increments message count
   - Updates timestamp

2. **Assistant Message** (src/session/mod.rs:111-131):
   - Creates a new message with "assistant" role
   - Includes content (if any) and tool calls
   - Generates unique UUID
   - Increments message count

3. **Tool Results** (src/session/mod.rs:133-149):
   - Added to the last assistant message
   - Links to specific tool call via ID
   - Includes output, observation, and status

### Retrieving Conversation History

The `get_conversation_history()` method (src/session/mod.rs:151-196):
- Converts internal message format to LLM provider format
- Handles user messages
- Handles assistant messages (including those with only tool calls)
- Converts tool results into user messages with combined observation + structured output
- Used when sending context to the LLM

## Key Points

- **Sessions are NOT saved by default** in non-interactive mode (requires `--save` flag)
- **Interactive mode** automatically saves after each exchange
- **All messages** including tool calls and tool results are preserved
- **System prompts and personas** are stored with the session
- **Sessions can be resumed** later using the session ID
- **Export to markdown** provides human-readable documentation
- **Tool execution results** are stored and can be reviewed later
- **Timestamps** track when messages were created and when the session was last updated

## File Examples

### Session JSON Structure

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "My Coding Session",
    "directory": "/path/to/project",
    "created_at": "2024-01-15T10:30:00Z",
    "updated_at": "2024-01-15T11:45:00Z",
    "message_count": 42,
    "system_prompt": "You are a helpful coding assistant...",
    "persona": "architect"
  },
  [
    {
      "id": "msg-001",
      "role": "user",
      "content": "Help me refactor this function",
      "tool_calls": [],
      "tool_results": [],
      "timestamp": "2024-01-15T10:31:00Z"
    },
    {
      "id": "msg-002",
      "role": "assistant",
      "content": "I'll help you refactor that function...",
      "tool_calls": [
        {
          "id": "call-123",
          "name": "file_search",
          "arguments": {"pattern": "my_function", "operation": "grep"}
        }
      ],
      "tool_results": [
        {
          "tool_call_id": "call-123",
          "output": {"matches": [...]},
          "observation": "Found 3 matches",
          "status": "success"
        }
      ],
      "timestamp": "2024-01-15T10:31:05Z"
    }
  ]
]
```

## Command-Line Interface

### Creating/Resuming Sessions

```bash
# Start interactive mode (prompts for session title)
codeagent --provider openai

# Start with specific title
codeagent --provider openai --session-title "Bug Fix Session"

# Resume existing session
codeagent --provider openai --session <session_id>

# Non-interactive with save
codeagent --provider openai --input "Fix the bug" --save --session-title "Bug Fix"
```

### Interactive Commands

During an interactive session:
- `exit` - Save session and quit
- `save` - Save session manually
- `export [filename]` - Export conversation to markdown
