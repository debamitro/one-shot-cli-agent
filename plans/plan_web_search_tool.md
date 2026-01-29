# Plan: Web Search Tool and Additional Tools

**Date:** 2026-01-29  
**POC:** CodeAgent Enhancement

## TL;DR

Add a web search tool to enable the agent to fetch real-time information from the web. Also propose additional useful tools (URL fetcher, git operations, package manager integration) to expand agent capabilities.

## Context

**Current Tool System:**
- Tool trait: `name()`, `description()`, `input_schema()`, `execute() -> ToolOutput`
- Registry pattern: Tools registered in `ToolRegistry::new()` in `src/tools/mod.rs`
- Existing tools: FileSearchTool, EditFileTool, BashTool
- Tool schemas sent to LLM as JSON, tool calls executed synchronously

**Architecture:**
- Rust-based CLI with OpenAI/Anthropic LLM providers
- Session management with persistent conversation history
- REPL loop that executes tools and feeds results back to LLM

## Proposed: Web Search Tool

### Implementation Approach

**1. API Selection**
- **Recommended:** Serper API (https://serper.dev)
  - Clean JSON API, generous free tier
  - Supports Google Search, News, Images, Scholar
  - Simple REST interface
- **Alternative:** SerpAPI, Bing Search API

**2. Dependencies to Add**
```toml
# Cargo.toml additions
reqwest = { version = "0.11", features = ["json"] }
tokio = { version = "1", features = ["full"] }  # Likely already present
serde = { version = "1.0", features = ["derive"] }
```

**3. Tool Structure**

```rust
// src/tools/web_search.rs
pub struct WebSearchTool {
    api_key: String,
}

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using Google search. Returns top results with titles, snippets, and URLs."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5, max: 10)"
                },
                "search_type": {
                    "type": "string",
                    "enum": ["web", "news", "images"],
                    "description": "Type of search (default: web)"
                }
            },
            "required": ["query"]
        })
    }

    fn execute(&self, input: serde_json::Value) -> Result<ToolOutput> {
        // Make async HTTP request to Serper API
        // Parse response and format results
    }
}
```

**4. API Key Management**
- Environment variable: `SERPER_API_KEY` or `WEB_SEARCH_API_KEY`
- Pass to tool during registration in `ToolRegistry::new()`
- Graceful fallback: If no key present, tool returns error in execute()

**5. Response Format**
```json
{
  "output": {
    "results": [
      {
        "title": "Result title",
        "url": "https://example.com",
        "snippet": "Description...",
        "position": 1
      }
    ],
    "query": "original query",
    "total_results": 5
  },
  "observation": "Found 5 results for 'rust async programming'",
  "display": "1. Result title\n   https://example.com\n   Description...\n\n2. ...",
  "status": "success"
}
```

**6. Async Handling**
- Tool `execute()` is currently synchronous
- Need to either:
  - Add `tokio::runtime::Runtime::new()` in execute() and block on async request
  - OR refactor Tool trait to support async (larger change)
- **Recommendation:** Use blocking runtime for now (simpler)

### Files to Modify

1. **Create:** `src/tools/web_search.rs` (new file, ~150-200 lines)
2. **Modify:** `src/tools/mod.rs`
   - Add `pub mod web_search;` (line 3)
   - Register tool in `ToolRegistry::new()` (after line 44)
3. **Modify:** `Cargo.toml`
   - Add `reqwest` dependency
4. **Modify:** `src/main.rs`
   - Read `SERPER_API_KEY` from env
   - Pass to ToolRegistry or WebSearchTool constructor

### Steps

1. Add dependencies to Cargo.toml
2. Create src/tools/web_search.rs with Tool implementation
3. Add module declaration and registration in mod.rs
4. Update main.rs to handle API key
5. Test with sample queries
6. Document usage in README/AGENTS.md

---

## Additional Useful Tools

### 1. URL Fetch Tool
**Purpose:** Fetch and extract content from web pages (for research, documentation reading)

**Implementation:**
- Use `reqwest` to fetch HTML
- Parse with `scraper` crate or convert to markdown with `html2md`
- Support PDF extraction via `pdf-extract` crate

**Schema:**
```rust
{
  "url": "string (required)",
  "format": "text | markdown | raw",
  "max_length": "integer (optional, default: 10000 chars)"
}
```

**Use Cases:**
- Read documentation from URLs
- Fetch GitHub issue/PR content
- Extract article text for summarization

---

### 2. Git Operations Tool
**Purpose:** Structured git commands (beyond raw bash)

**Why Needed:**
- Parse git output reliably (JSON-friendly)
- Common operations: status, diff, log, blame
- Safety checks (prevent destructive ops without confirmation)

**Operations:**
- `git_status`: Structured status output (staged, unstaged, untracked files)
- `git_diff`: Pretty diff with file paths and change summaries
- `git_log`: Commit history with messages, authors, dates
- `git_blame`: Line-by-line authorship for files

**Implementation:**
- Use `git2-rs` crate (libgit2 bindings) for programmatic git access
- OR parse `git` CLI output (simpler but less robust)

---

### 3. Package Manager Tool
**Purpose:** Query and manage dependencies (Cargo, npm, pip, etc.)

**Operations:**
- `list_dependencies`: Show current dependencies from manifest
- `search_package`: Search package registry
- `get_package_info`: Get package metadata (versions, description)
- `check_updates`: Find outdated dependencies

**Implementation:**
- Cargo: Parse `Cargo.toml`, use `cargo search` API
- npm: Use `npm search` and `npm info`
- Detect project type from working directory

**Value:**
- Agent can understand project dependencies
- Suggest updates or alternatives
- Add dependencies with proper versions

---

### 4. Code Analysis Tool (AST)
**Purpose:** Parse and analyze code structure

**Operations:**
- `parse_file`: Get AST representation
- `find_functions`: List all function definitions
- `find_imports`: Extract import statements
- `get_symbols`: Find classes, structs, enums

**Implementation:**
- Rust: Use `syn` crate for parsing
- Multi-language: Use `tree-sitter` (supports many languages)

**Value:**
- Understand code structure without reading entire files
- Find specific functions/classes across large codebases
- Better code navigation and refactoring suggestions

---

### 5. HTTP Request Tool
**Purpose:** Make arbitrary HTTP requests (REST APIs, webhooks)

**Operations:**
- GET, POST, PUT, DELETE requests
- Custom headers, query params, JSON bodies
- Authentication support (Bearer tokens, API keys)

**Schema:**
```rust
{
  "url": "string",
  "method": "GET | POST | PUT | DELETE",
  "headers": "object (optional)",
  "body": "string | object (optional)",
  "auth": "Bearer <token> | ApiKey <key>"
}
```

**Use Cases:**
- Interact with GitHub API, Jira, Slack, etc.
- Test APIs during development
- Fetch data from external services

---

## Priority Ranking

1. **Web Search Tool** (HIGH) - Enables real-time information access
2. **URL Fetch Tool** (HIGH) - Complements web search, reads documentation
3. **Git Operations Tool** (MEDIUM) - Improves git workflow reliability
4. **HTTP Request Tool** (MEDIUM) - Broad API integration capabilities
5. **Package Manager Tool** (LOW) - Nice-to-have for dependency management
6. **Code Analysis Tool** (LOW) - Advanced feature, requires heavy parsing

---

## Open Questions

1. **Async handling:** Should we refactor Tool trait to be async, or use blocking runtime?
2. **API key storage:** Environment variables only, or support config file?
3. **Rate limiting:** Should tools implement rate limiting for web APIs?
4. **Caching:** Should web search/URL fetch cache results per session?
5. **Tool composition:** Should tools be able to call other tools internally?

---

## Next Steps

1. **Implement Web Search Tool:**
   - Add dependencies
   - Create web_search.rs
   - Register and test

2. **Add URL Fetch Tool:**
   - Reuse HTTP client from web search
   - Add HTML parsing

3. **Document new tools:**
   - Update AGENTS.md with usage examples
   - Add to README

4. **Consider async refactor:**
   - Evaluate if blocking approach causes issues
   - Plan trait refactor if needed
