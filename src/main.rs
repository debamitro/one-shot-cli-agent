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

    #[arg(short, long, help = "API key (or set OPENAI_API_KEY/ANTHROPIC_API_KEY env var)")]
    api_key: Option<String>,

    #[arg(short, long, help = "Model to use (optional, uses provider default)")]
    model: Option<String>,

    #[arg(short = 'd', long, help = "Working directory", default_value = ".")]
    directory: String,

    #[arg(short = 's', long, help = "Session ID to resume")]
    session: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Get API key from args or environment
    let api_key = args.api_key.or_else(|| {
        match args.provider.as_str() {
            "openai" => std::env::var("OPENAI_API_KEY").ok(),
            "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
            _ => None,
        }
    }).ok_or_else(|| anyhow::anyhow!("API key required. Set via --api-key or environment variable"))?;

    // Create provider
    let provider: Box<dyn LLMProvider> = match args.provider.as_str() {
        "openai" => Box::new(provider::openai::OpenAIProvider::new(api_key, args.model)),
        "anthropic" => Box::new(provider::anthropic::AnthropicProvider::new(api_key, args.model)),
        _ => return Err(anyhow::anyhow!("Unknown provider: {}. Use 'openai' or 'anthropic'", args.provider)),
    };

    // Setup storage
    let storage_path = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
        .join(".codeagent")
        .join("sessions");

    // Load or create session
    let mut session = if let Some(session_id) = args.session {
        println!("{}", format!("Resuming session: {}", session_id).cyan());
        Session::load(&session_id, storage_path)?
    } else {
        let title: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Session title")
            .default("New Coding Session".to_string())
            .interact_text()?;

        let session = Session::new(title, args.directory, storage_path);
        println!("{}", format!("Created session: {}", session.info.id).green());
        session
    };

    // Initialize tools
    let tool_registry = ToolRegistry::new();
    let tool_definitions: Vec<serde_json::Value> = tool_registry
        .list_definitions()
        .iter()
        .map(|def| serde_json::to_value(def).unwrap())
        .collect();

    // Print welcome message
    println!("\n{}", "CodeAgent - Interactive Coding Assistant".bold().cyan());
    println!("{}", format!("Provider: {} | Directory: {}", args.provider, session.info.directory).dimmed());
    println!("{}", "Type 'exit' to quit, 'save' to save session\n".dimmed());

    // Main REPL loop
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
            "" => continue,
            _ => {}
        }

        // Add user message
        session.add_user_message(user_input);

        // Get conversation history with system prompt
        let mut messages = vec![Message {
            role: "system".to_string(),
            content: "You are a helpful coding assistant. You have access to tools for file operations, code search, and command execution. Use them to help the user with their coding tasks.".to_string(),
        }];
        messages.extend(session.get_conversation_history());

        // Agent loop - continue until no tool calls
        loop {
            println!("{}", "\nAssistant: ".bold().blue());

            // Stream completion
            let mut rx = provider.stream_completion(messages.clone(), Some(tool_definitions.clone())).await?;

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
            let content = if full_content.is_empty() { None } else { Some(full_content) };
            session.add_assistant_message(content.clone(), all_tool_calls.clone());

            // If no tool calls, we're done
            if all_tool_calls.is_empty() {
                break;
            }

            // Execute tool calls
            println!("\n{}", "Executing tools...".yellow());
            for tool_call in &all_tool_calls {
                println!("  {} {}", "→".blue(), tool_call.name.bold());
                
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
        }

        // Save after each interaction
        session.save()?;
    }

    Ok(())
}