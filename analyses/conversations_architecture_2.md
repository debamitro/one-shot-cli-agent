# Sessions Architecture

## How Sessions Are Saved

Sessions in this project are saved using the following mechanism:

### Storage Location
- **Storage Path**: Sessions are stored in a `sessions` directory within the project structure (see src/main.rs:161)
- **File Format**: Each session is saved as a JSON file named `{session_id}.json`

### Save Function Implementation
The core save logic is in **src/session/mod.rs:85-91**:

```rust
pub fn save(&self) -> Result<()> {
    std::fs::create_dir_all(&self.storage_path)?;
    let session_file = self.storage_path.join(format!("{}.json", self.info.id));
    let data = serde_json::to_string_pretty(&(&self.info, &self.messages))?;
    std::fs::write(session_file, data)?;
    Ok(())
}
```

### What Gets Saved
Each session file contains a tuple of:
1. **SessionInfo** - Metadata including:
   - `id`: Unique UUID identifier
   - `title`: Session title
   - `directory`: Working directory path
   - `created_at` and `updated_at`: Timestamps
   - `message_count`: Number of messages
   - `system_prompt`: Optional system prompt
   - `persona`: Optional persona setting

2. **Messages** - Vector of `MessagePart` objects containing:
   - Message ID, role, content
   - Tool calls and their results
   - Timestamps

### When Sessions Are Saved

There are **four different scenarios** where sessions are saved:

1. **Interactive Mode - Manual Save** (src/main.rs:307-309)
   - When user types `save` command in the REPL

2. **Interactive Mode - Exit** (src/main.rs:302-305)
   - When user types `exit` command in the REPL

3. **Interactive Mode - Auto-save** (src/main.rs:560)
   - Automatically saved after each interaction in the main agent loop

4. **Non-interactive Mode - Conditional Save** (src/main.rs:765-768)
   - Only saved if the `--save` flag is provided
   - Sessions are NOT saved by default in non-interactive mode (as noted in src/main.rs:80)

### Loading Sessions
Sessions can be loaded using the `Session::load()` method at **src/session/mod.rs:73-82**, which reads the JSON file and deserializes it back into a Session object.
