# Plan: System Prompt Override Feature

**Date:** 2026-01-31  
**Context:** Add ability to override the hardcoded system prompt

## TL;DR

Add CLI options to override the default system prompt via:
1. Direct string argument (`--system-prompt "..."`)
2. File path (`--system-prompt-file path/to/prompt.txt`)

Store custom prompts in sessions for persistence across resumes.

## Current State

- System prompt hardcoded in `src/main.rs` lines 174-177
- Default: "You are a helpful coding assistant. You have access to tools for file operations, code search, and command execution. Use them to help the user with their coding tasks."
- No way to customize without editing source code

## Proposed Approach

### 1. CLI Arguments (in `src/main.rs`)

Add two new arguments to `Args` struct:
- `--system-prompt <text>`: Direct string override
- `--system-prompt-file <path>`: Read prompt from file

Priority: `--system-prompt` > `--system-prompt-file` > default

### 2. Session Storage

Extend `SessionInfo` or create new field in session JSON to store:
- `custom_system_prompt: Option<String>`

When loading existing session:
- Use stored custom prompt if present
- CLI args can still override on resume

### 3. Implementation Steps

1. **Update CLI args** (`src/main.rs`):
   - Add `system_prompt: Option<String>` field
   - Add `system_prompt_file: Option<PathBuf>` field
   - Parse and validate (file must exist if path provided)

2. **Add session field** (`src/session/mod.rs`):
   - Add `system_prompt: Option<String>` to `SessionInfo` struct
   - Update serialization/deserialization

3. **Prompt resolution logic** (`src/main.rs`):
   ```rust
   let system_prompt = if let Some(prompt) = args.system_prompt {
       prompt
   } else if let Some(path) = args.system_prompt_file {
       std::fs::read_to_string(path)?
   } else if let Some(prompt) = session.get_system_prompt() {
       prompt
   } else {
       DEFAULT_SYSTEM_PROMPT.to_string()
   };
   ```

4. **Store in session**:
   - Save resolved prompt to session when creating new session
   - Update on CLI override during resume

### Files to Modify

- `src/main.rs`: CLI args, prompt resolution, usage in message creation
- `src/session/mod.rs`: Add `system_prompt` field, getter method

## Benefits

- No need to recompile for different prompts
- Experiment with prompts easily
- Session-specific customization
- File-based prompts enable version control and sharing

## Open Questions

- Should we validate prompt length/format?
- Should we expose the current prompt via a command (e.g., `show-prompt`)?
- Should we allow updating prompt mid-session?
