# Tool Execution Loop Analysis

## Problem Description
The agent continues calling tools in an infinite loop even after successful execution, never reaching a final answer state.

## Root Cause

### Current Implementation (src/main.rs lines 264-368, 392-504)
```rust
loop {
    // ALWAYS passes tool_definitions, regardless of iteration
    let mut rx = provider.stream_completion(
        messages.clone(), 
        Some(tool_definitions.clone())  // ← PROBLEM: Tools always available
    ).await?;
    
    // Collect response and tool calls
    let mut all_tool_calls = Vec::new();
    while let Some(chunk) = rx.recv().await {
        all_tool_calls.extend(chunk.tool_calls);
    }
    
    // Only break condition: no tool calls
    if all_tool_calls.is_empty() {
        break;  // ← Only exit path
    }
    
    // Execute tools and add results
    for tool_call in &all_tool_calls {
        let result = tool_registry.execute(...)?;
        messages.push(Message {
            role: "user",
            content: format!("Tool '{}' result: {}", tool_call.name, observation),
        });
    }
    // Loop continues with tools still available ← PROBLEM
}
```

### Why This Causes Infinite Loops

1. **Tools Always Available**: Every iteration passes the full tool registry to the LLM, signaling "you can still use tools"
2. **No Completion Signal**: The LLM has no way to distinguish between:
   - "Initial request - use tools to help"
   - "Tools executed successfully - now synthesize final answer"
3. **Ambiguous Context**: After successful tool execution, the LLM sees:
   - Tool results in conversation
   - Tools still available for use
   - No clear "you're done, give final answer" signal
4. **LLM Behavior**: Modern LLMs tend to:
   - Over-verify results by calling tools again
   - Request additional context "just to be thorough"
   - Continue using tools when they remain available
   - Struggle to determine when to stop tool usage

## Evidence from Code

### Provider Implementations
Both providers accept tools on every call:
- `openai.rs` line 143: `tools: Option<Vec<serde_json::Value>>`
- `anthropic.rs` line 173: `tools: Option<Vec<serde_json::Value>>`

No special handling for "final iteration" or "stop using tools" signals.

### Loop Control
Only one break condition exists (line 298/426):
```rust
if all_tool_calls.is_empty() {
    break;
}
```

No alternative exit conditions:
- No iteration counter/limit
- No success state detection
- No "task complete" signal
- No finish_reason checking

## Solution Options

### Option 1: One-Shot Tool Usage (Simplest)
Pass tools only on first iteration:
```rust
let mut iteration = 0;
loop {
    let tools = if iteration == 0 {
        Some(tool_definitions.clone())
    } else {
        None  // No tools after first iteration
    };
    
    let mut rx = provider.stream_completion(messages.clone(), tools).await?;
    iteration += 1;
    // ... rest of loop
}
```

**Pros**: Simple, prevents loops, forces LLM to synthesize answer after one tool call
**Cons**: Limits multi-step reasoning (can't chain tools)

### Option 2: Iteration Limit (Balanced)
Allow N iterations with tools, then force final answer:
```rust
const MAX_TOOL_ITERATIONS: usize = 5;
let mut iteration = 0;

loop {
    let tools = if iteration < MAX_TOOL_ITERATIONS {
        Some(tool_definitions.clone())
    } else {
        None  // Force final answer after N iterations
    };
    
    let mut rx = provider.stream_completion(messages.clone(), tools).await?;
    
    // ... collect tool calls ...
    
    if all_tool_calls.is_empty() || iteration >= MAX_TOOL_ITERATIONS {
        break;
    }
    
    iteration += 1;
    // ... execute tools ...
}
```

**Pros**: Allows multi-step tool usage, prevents infinite loops, configurable limit
**Cons**: Arbitrary limit, may cut off legitimate multi-step tasks

### Option 3: Finish Tool (Most Robust)
Add a "finish" tool that LLM calls when done:
```rust
// Add to tool registry
pub struct FinishTool;
impl Tool for FinishTool {
    fn name(&self) -> &str { "finish" }
    fn description(&self) -> &str {
        "Call this when you have completed the task and want to provide a final answer. \
         Include your complete response in the 'answer' parameter."
    }
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "answer": {
                    "type": "string",
                    "description": "Your final answer to the user"
                }
            },
            "required": ["answer"]
        })
    }
    fn execute(&self, args: serde_json::Value) -> Result<ToolOutput> {
        let answer = args.get("answer")
            .and_then(|v| v.as_str())
            .unwrap_or("Task complete");
        Ok(ToolOutput {
            output: json!({"answer": answer}),
            observation: answer.to_string(),
            display: None,
            status: "success".to_string(),
        })
    }
}

// In loop
for tool_call in &all_tool_calls {
    if tool_call.name == "finish" {
        // Extract final answer and exit
        if let Some(answer) = tool_call.arguments.get("answer") {
            println!("\n{}", answer.as_str().unwrap_or(""));
        }
        return Ok(());  // Exit agent loop
    }
    // ... execute other tools ...
}
```

**Pros**: 
- Explicit completion signal
- LLM controls when to stop
- Allows unlimited tool iterations if needed
- Clean separation of "working" vs "done" states

**Cons**: 
- Requires LLM to learn to use finish tool
- Adds one more tool to registry
- LLM might forget to call it

### Option 4: Stop Reason Analysis (Provider-Dependent)
Check provider's stop reason:
```rust
// In streaming loop
let mut stop_reason: Option<String> = None;

while let Some(chunk) = rx.recv().await {
    if chunk.finished {
        // Extract stop reason from provider response
        // OpenAI: finish_reason (stop, tool_calls, length, etc.)
        // Anthropic: stop_reason (end_turn, max_tokens, tool_use, etc.)
        stop_reason = Some(extract_stop_reason(&chunk));
    }
}

// After collecting tool calls
if all_tool_calls.is_empty() {
    break;  // Normal completion
}

if stop_reason == Some("stop") && !all_tool_calls.is_empty() {
    // LLM returned both tool calls and stop signal
    // This is unusual - might indicate LLM confusion
    println!("Warning: Received stop signal with tool calls");
}
```

**Pros**: Uses provider signals
**Cons**: 
- Requires modifying StreamChunk to include stop_reason
- Different semantics across providers
- Doesn't solve core issue

## Recommended Solution: Iteration Limit (Option 2)

**Rationale**:
1. **Prevents infinite loops**: Hard cap on iterations
2. **Allows multi-step reasoning**: LLM can chain tools if needed
3. **Simple to implement**: Minimal code changes
4. **User-friendly**: Fails gracefully with message "Max iterations reached"
5. **Configurable**: Can adjust limit based on use case

**Implementation**:
1. Add iteration counter to main loop
2. Pass tools conditionally based on counter
3. Add break condition for max iterations
4. Optional: Add CLI flag `--max-tool-iterations` for user control

**Follow-up Enhancement**:
After iteration limit proves stable, consider adding Option 3 (finish tool) for more sophisticated control.

## Testing Recommendations

1. **Test infinite loop scenarios**:
   - Request file read → LLM requests same file again
   - Request search → LLM requests clarifying search
   - Request bash command → LLM requests verification command

2. **Test multi-step tasks**:
   - "Find all TODO comments and count them" (grep → bash/analysis)
   - "Read config, modify value, write back" (read → edit → read)
   - "Search for function, read file, suggest improvements" (search → read → analysis)

3. **Test edge cases**:
   - Tool execution errors
   - Empty tool results
   - LLM requesting unavailable tools

## Related Issues to Consider

1. **Message history growth**: Each iteration adds messages. Consider compaction.
2. **Token usage**: Unlimited iterations = unbounded cost. Add token tracking.
3. **Error propagation**: Failed tools should probably stop iteration, not continue.
4. **User interruption**: Add Ctrl+C handling to break agent loop.
