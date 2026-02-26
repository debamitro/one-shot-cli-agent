mod provider;
mod session;
mod tools;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Input};

use provider::{LLMProvider, Message};
use session::Session;
use tools::ToolRegistry;

#[derive(Parser, Debug)]
#[command(name = "codeagent")]
#[command(about = "Interactive coding agent with OpenAI and Anthropic support", long_about = None)]
struct Args {
    #[arg(short, long, help = "Provider to use: openai or anthropic")]
    provider: String,

    #[arg(
        short,
        long,
        help = "API key (or set OPENAI_API_KEY/ANTHROPIC_API_KEY env var)"
    )]
    api_key: Option<String>,

    #[arg(short, long, help = "Model to use (optional, uses provider default)")]
    model: Option<String>,

    #[arg(short = 'd', long, help = "Working directory", default_value = ".")]
    directory: String,

    #[arg(short = 's', long, help = "Session ID to resume")]
    session: Option<String>,

    #[arg(long, help = "OpenAI base URL (optional, overrides default)")]
    openai_base_url: Option<String>,

    #[arg(long, help = "Anthropic base URL (optional, overrides default)")]
    anthropic_base_url: Option<String>,

    #[arg(long, help = "System prompt override (direct text)")]
    system_prompt: Option<String>,

    #[arg(long, help = "System prompt override (read from file)")]
    system_prompt_file: Option<String>,

    #[arg(
        short = 'i',
        long,
        help = "Input for non-interactive mode (single task to execute)"
    )]
    input: Option<String>,

    #[arg(
        long,
        help = "Save session in non-interactive mode (sessions not saved by default)"
    )]
    save: bool,

    #[arg(
        long,
        help = "Session title for non-interactive mode (auto-generated if not provided)"
    )]
    session_title: Option<String>,

    #[arg(
        long,
        help = "Auto-approve all bash commands (non-interactive mode only, use with caution)"
    )]
    auto_approve: bool,

    #[arg(
        long,
        help = "Maximum tool call iterations before forcing final answer (default: 5)",
        default_value = "5"
    )]
    max_tool_iterations: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Detect mode: interactive (default) vs non-interactive (--input provided)
    let is_interactive = args.input.is_none();

    // Get API key from args or environment
    let api_key = args
        .api_key
        .or_else(|| match args.provider.as_str() {
            "openai" => std::env::var("OPENAI_API_KEY").ok(),
            "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
            _ => None,
        })
        .ok_or_else(|| {
            anyhow::anyhow!("API key required. Set via --api-key or environment variable")
        })?;

    // Create provider
    let provider: Box<dyn LLMProvider> = match args.provider.as_str() {
        "openai" => Box::new(provider::openai::OpenAIProvider::new(
            api_key,
            args.model,
            args.openai_base_url,
        )),
        "anthropic" => Box::new(provider::anthropic::AnthropicProvider::new(
            api_key,
            args.model,
            args.anthropic_base_url,
        )),
        _ => {
            return Err(anyhow::anyhow!(
                "Unknown provider: {}. Use 'openai' or 'anthropic'",
                args.provider
            ))
        }
    };

    // Setup storage
    let storage_path = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
        .join(".codeagent")
        .join("sessions");

    // Resolve system prompt (priority: CLI arg > file > default)
    const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful coding assistant. You have access to tools for file operations, code search, and command execution. Use them to help the user with their coding tasks.";

    let resolved_prompt = if let Some(prompt) = args.system_prompt {
        Some(prompt)
    } else if let Some(file_path) = args.system_prompt_file {
        let content = std::fs::read_to_string(&file_path).map_err(|e| {
            anyhow::anyhow!("Failed to read system prompt file '{}': {}", file_path, e)
        })?;
        Some(content)
    } else {
        None
    };

    // Load or create session
    let mut session = if let Some(session_id) = args.session {
        println!("{}", format!("Resuming session: {}", session_id).cyan());
        let mut session = Session::load(&session_id, storage_path)?;

        // Override session prompt if CLI arg provided
        if resolved_prompt.is_some() {
            session.set_system_prompt(resolved_prompt.clone());
        }

        session
    } else {
        let title = if is_interactive {
            // Interactive mode: prompt user
            Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Session title")
                .default("New Coding Session".to_string())
                .interact_text()?
        } else {
            // Non-interactive mode: use CLI arg or auto-generate
            args.session_title.unwrap_or_else(|| {
                let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
                format!("Batch_{}", timestamp)
            })
        };

        let session = Session::new(title, args.directory, storage_path, resolved_prompt.clone());
        println!(
            "{}",
            format!("Created session: {}", session.info.id).green()
        );
        session
    };

    // Get final system prompt (session stored > default)
    let system_prompt = session
        .get_system_prompt()
        .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string());

    // Initialize tools
    let web_search_api_key = std::env::var("SERPER_API_KEY").ok();
    let tool_registry = ToolRegistry::new_with_api_keys(web_search_api_key);
    let tool_definitions: Vec<serde_json::Value> = tool_registry
        .list_definitions()
        .iter()
        .map(|def| serde_json::to_value(def).unwrap())
        .collect();

    // Print welcome message
    if is_interactive {
        println!(
            "\n{}",
            "CodeAgent - Interactive Coding Assistant".bold().cyan()
        );
        println!(
            "{}",
            format!(
                "Provider: {} | Directory: {}",
                args.provider, session.info.directory
            )
            .dimmed()
        );
        println!(
            "{}",
            "Type 'exit' to quit, 'save' to save session, 'export [file]' to export as markdown\n"
                .dimmed()
        );
    } else {
        println!(
            "{}",
            format!(
                "CodeAgent (non-interactive) | Provider: {} | Directory: {}",
                args.provider, session.info.directory
            )
            .dimmed()
        );
    }

    // Main execution: interactive REPL or non-interactive single run
    if is_interactive {
        // Interactive REPL mode
        loop {
            let user_input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("You")
                .interact_text()?;

            match user_input.trim() {
                "exit" => {
                    session.save()?;
                    println!("{}", "Session saved. Goodbye!".green());
                    break;
                }
                "save" => {
                    session.save()?;
                    println!("{}", "Session saved.".green());
                    continue;
                }
                input if input.starts_with("export") => {
                    // Parse optional filename argument
                    let filename = input
                        .strip_prefix("export")
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());

                    match session.export_to_markdown(filename) {
                        Ok(path) => {
                            println!("{}", format!("Exported to: {}", path).green());
                        }
                        Err(e) => {
                            println!("{}", format!("Export failed: {}", e).red());
                        }
                    }
                    continue;
                }
                "" => continue,
                _ => {}
            }

            // Add user message
            session.add_user_message(user_input);

            // Get conversation history with system prompt
            let mut messages = vec![Message {
                role: "system".to_string(),
                content: system_prompt.clone(),
            }];
            messages.extend(session.get_conversation_history());

            // Agent loop - continue until no tool calls or max iterations
            let mut iteration = 0;
            loop {
                println!("{}", "\nAssistant: ".bold().blue());

                // Conditionally pass tools based on iteration count
                let tools = if iteration < args.max_tool_iterations {
                    Some(tool_definitions.clone())
                } else {
                    None
                };

                // Stream completion
                let mut rx = provider
                    .stream_completion(messages.clone(), tools)
                    .await?;

                let mut full_content = String::new();
                let mut all_tool_calls = Vec::new();

                while let Some(chunk) = rx.recv().await {
                    if let Some(content) = chunk.content {
                        print!("{}", content);
                        full_content.push_str(&content);
                    }

                    all_tool_calls.extend(chunk.tool_calls);

                    if chunk.finished {
                        break;
                    }
                }
                println!(); // Newline after streaming

                // Add assistant message
                let content = if full_content.is_empty() {
                    None
                } else {
                    Some(full_content)
                };
                session.add_assistant_message(content.clone(), all_tool_calls.clone());

                // If no tool calls, we're done
                if all_tool_calls.is_empty() {
                    break;
                }

                // Check if max iterations reached
                if iteration >= args.max_tool_iterations {
                    println!(
                        "\n{}",
                        format!(
                            "⚠ Maximum tool iterations ({}) reached. Forcing final answer.",
                            args.max_tool_iterations
                        )
                        .yellow()
                    );
                    break;
                }

                // Execute tool calls
                println!("\n{}", "Executing tools...".yellow());
                let mut finish_called = false;
                for tool_call in &all_tool_calls {
                    println!("  {} {}", "→".blue(), tool_call.name.bold());

                    // Check if this is the finish tool
                    if tool_call.name == "finish" {
                        finish_called = true;
                    }

                    // Print command details for bash tool
                    if tool_call.name == "bash" {
                        if let Some(cmd) = tool_call.arguments.get("command") {
                            if let Some(cmd_str) = cmd.as_str() {
                                println!("    Command: {}", cmd_str.dimmed());
                            }
                        }
                        if let Some(desc) = tool_call.arguments.get("description") {
                            if let Some(desc_str) = desc.as_str() {
                                println!("    Description: {}", desc_str.dimmed());
                            }
                        }
                        if let Some(cwd) = tool_call.arguments.get("cwd") {
                            if let Some(cwd_str) = cwd.as_str() {
                                println!("    Working directory: {}", cwd_str.dimmed());
                            }
                        }
                    }

                    match tool_registry.execute(&tool_call.name, tool_call.arguments.clone()) {
                        Ok(result) => {
                            println!("    {}", result.observation.green());
                            if let Some(display) = &result.display {
                                if !display.is_empty() {
                                    println!("\n{}\n", display.dimmed());
                                }
                            }

                            let observation = result.observation.clone();

                            session.add_tool_result(
                                tool_call.id.clone(),
                                result.output,
                                observation.clone(),
                                result.status,
                            );

                            // Add tool result to messages for next iteration
                            messages.push(Message {
                                role: "user".to_string(),
                                content: format!("Tool '{}' result: {}", tool_call.name, observation),
                            });
                        }
                        Err(e) => {
                            let error_msg = format!("Tool execution failed: {}", e);
                            println!("    {}", error_msg.red());

                            session.add_tool_result(
                                tool_call.id.clone(),
                                serde_json::json!({"error": error_msg}),
                                error_msg.clone(),
                                "error".to_string(),
                            );

                            messages.push(Message {
                                role: "user".to_string(),
                                content: format!("Tool '{}' failed: {}", tool_call.name, error_msg),
                            });
                        }
                    }
                }

                // If finish tool was called, exit the agent loop
                if finish_called {
                    break;
                }

                // Increment iteration counter
                iteration += 1;
            }

            // Save after each interaction
            session.save()?;
        }
    } else {
        // Non-interactive mode
        let user_input = args.input.unwrap(); // Safe: checked is_interactive

        if user_input.is_empty() {
            return Err(anyhow::anyhow!("Input cannot be empty for non-interactive mode"));
        }

        // Add user message
        session.add_user_message(user_input);

        // Get conversation history with system prompt
        let mut messages = vec![Message {
            role: "system".to_string(),
            content: system_prompt.clone(),
        }];
        messages.extend(session.get_conversation_history());

        // Agent loop - continue until no tool calls or max iterations
        let mut iteration = 0;
        loop {
            println!("{}", "\nAssistant: ".bold().blue());

            // Conditionally pass tools based on iteration count
            let tools = if iteration < args.max_tool_iterations {
                Some(tool_definitions.clone())
            } else {
                None
            };

            // Stream completion
            let mut rx = provider
                .stream_completion(messages.clone(), tools)
                .await?;

            let mut full_content = String::new();
            let mut all_tool_calls = Vec::new();

            while let Some(chunk) = rx.recv().await {
                if let Some(content) = chunk.content {
                    print!("{}", content);
                    full_content.push_str(&content);
                }

                all_tool_calls.extend(chunk.tool_calls);

                if chunk.finished {
                    break;
                }
            }
            println!(); // Newline after streaming

            // Add assistant message
            let content = if full_content.is_empty() {
                None
            } else {
                Some(full_content)
            };
            session.add_assistant_message(content.clone(), all_tool_calls.clone());

            // If no tool calls, we're done
            if all_tool_calls.is_empty() {
                break;
            }

            // Check if max iterations reached
            if iteration >= args.max_tool_iterations {
                println!(
                    "\n{}",
                    format!(
                        "⚠ Maximum tool iterations ({}) reached. Forcing final answer.",
                        args.max_tool_iterations
                    )
                    .yellow()
                );
                break;
            }

            // Execute tool calls
            println!("\n{}", "Executing tools...".yellow());
            let mut finish_called = false;
            for tool_call in &all_tool_calls {
                println!("  {} {}", "→".blue(), tool_call.name.bold());

                // Check if this is the finish tool
                if tool_call.name == "finish" {
                    finish_called = true;
                }

                // Handle auto-approve for bash tool
                let mut modified_args = tool_call.arguments.clone();
                if tool_call.name == "bash" && args.auto_approve {
                    if let Some(obj) = modified_args.as_object_mut() {
                        obj.insert("skip_approval".to_string(), serde_json::json!(true));
                    }
                }

                // Print command details for bash tool
                if tool_call.name == "bash" {
                    if let Some(cmd) = modified_args.get("command") {
                        if let Some(cmd_str) = cmd.as_str() {
                            println!("    Command: {}", cmd_str.dimmed());
                        }
                    }
                    if let Some(desc) = modified_args.get("description") {
                        if let Some(desc_str) = desc.as_str() {
                            println!("    Description: {}", desc_str.dimmed());
                        }
                    }
                    if let Some(cwd) = modified_args.get("cwd") {
                        if let Some(cwd_str) = cwd.as_str() {
                            println!("    Working directory: {}", cwd_str.dimmed());
                        }
                    }
                }

                match tool_registry.execute(&tool_call.name, modified_args) {
                    Ok(result) => {
                        println!("    {}", result.observation.green());
                        if let Some(display) = &result.display {
                            if !display.is_empty() {
                                println!("\n{}\n", display.dimmed());
                            }
                        }

                        let observation = result.observation.clone();

                        session.add_tool_result(
                            tool_call.id.clone(),
                            result.output,
                            observation.clone(),
                            result.status,
                        );

                        // Add tool result to messages for next iteration
                        messages.push(Message {
                            role: "user".to_string(),
                            content: format!("Tool '{}' result: {}", tool_call.name, observation),
                        });
                    }
                    Err(e) => {
                        let error_msg = format!("Tool execution failed: {}", e);
                        println!("    {}", error_msg.red());

                        session.add_tool_result(
                            tool_call.id.clone(),
                            serde_json::json!({"error": error_msg}),
                            error_msg.clone(),
                            "error".to_string(),
                        );

                        messages.push(Message {
                            role: "user".to_string(),
                            content: format!("Tool '{}' failed: {}", tool_call.name, error_msg),
                        });
                    }
                }
            }

            // If finish tool was called, exit the agent loop
            if finish_called {
                break;
            }

            // Increment iteration counter
            iteration += 1;
        }

        // Save session if --save flag set
        if args.save {
            session.save()?;
            println!("{}", "\nSession saved.".green());
        }
    }

    Ok(())
}
