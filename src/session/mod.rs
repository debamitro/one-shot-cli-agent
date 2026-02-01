use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::provider::{Message, ToolCall};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePart {
    pub id: String,
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub output: serde_json::Value,
    pub observation: String,
    pub status: String,
}

pub struct Session {
    pub info: SessionInfo,
    pub messages: Vec<MessagePart>,
    storage_path: PathBuf,
}

impl Session {
    pub fn new(
        title: String,
        directory: String,
        storage_path: PathBuf,
        system_prompt: Option<String>,
    ) -> Self {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        Self {
            info: SessionInfo {
                id,
                title,
                directory,
                created_at: now,
                updated_at: now,
                message_count: 0,
                system_prompt,
            },
            messages: Vec::new(),
            storage_path,
        }
    }

    pub fn load(session_id: &str, storage_path: PathBuf) -> Result<Self> {
        let session_file = storage_path.join(format!("{}.json", session_id));
        let data = std::fs::read_to_string(session_file)?;
        let (info, messages) = serde_json::from_str::<(SessionInfo, Vec<MessagePart>)>(&data)?;

        Ok(Self {
            info,
            messages,
            storage_path,
        })
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.storage_path)?;
        let session_file = self.storage_path.join(format!("{}.json", self.info.id));
        let data = serde_json::to_string_pretty(&(&self.info, &self.messages))?;
        std::fs::write(session_file, data)?;
        Ok(())
    }

    pub fn add_user_message(&mut self, content: String) -> String {
        let message_id = Uuid::new_v4().to_string();

        self.messages.push(MessagePart {
            id: message_id.clone(),
            role: "user".to_string(),
            content: Some(content),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            timestamp: Utc::now(),
        });

        self.info.message_count += 1;
        self.info.updated_at = Utc::now();

        message_id
    }

    pub fn add_assistant_message(
        &mut self,
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    ) -> String {
        let message_id = Uuid::new_v4().to_string();

        self.messages.push(MessagePart {
            id: message_id.clone(),
            role: "assistant".to_string(),
            content,
            tool_calls,
            tool_results: Vec::new(),
            timestamp: Utc::now(),
        });

        self.info.message_count += 1;
        self.info.updated_at = Utc::now();

        message_id
    }

    pub fn add_tool_result(
        &mut self,
        tool_call_id: String,
        output: serde_json::Value,
        observation: String,
        status: String,
    ) {
        if let Some(last_message) = self.messages.last_mut() {
            last_message.tool_results.push(ToolResult {
                tool_call_id,
                output,
                observation,
                status,
            });
        }
        self.info.updated_at = Utc::now();
    }

    pub fn get_conversation_history(&self) -> Vec<Message> {
        let mut history = Vec::new();

        for msg in &self.messages {
            match msg.role.as_str() {
                "user" => {
                    if let Some(content) = &msg.content {
                        history.push(Message {
                            role: "user".to_string(),
                            content: content.clone(),
                        });
                    }
                }
                "assistant" => {
                    if let Some(content) = &msg.content {
                        history.push(Message {
                            role: "assistant".to_string(),
                            content: content.clone(),
                        });
                    }

                    for result in &msg.tool_results {
                        history.push(Message {
                            role: "user".to_string(),
                            content: format!("Tool result: {}", result.observation),
                        });
                    }
                }
                _ => {}
            }
        }

        history
    }

    pub fn get_system_prompt(&self) -> Option<String> {
        self.info.system_prompt.clone()
    }

    pub fn set_system_prompt(&mut self, prompt: Option<String>) {
        self.info.system_prompt = prompt;
        self.info.updated_at = Utc::now();
    }

    pub fn export_to_markdown(&self, filename: Option<String>) -> Result<String> {
        // Generate filename if not provided
        let filename = filename.unwrap_or_else(|| {
            let sanitized_title = sanitize_filename(&self.info.title);
            let id_prefix = get_id_prefix(&self.info.id);
            format!("{}_{}.md", sanitized_title, id_prefix)
        });

        // Resolve path relative to session directory
        let file_path = if PathBuf::from(&filename).is_absolute() {
            PathBuf::from(&filename)
        } else {
            PathBuf::from(&self.info.directory).join(&filename)
        };

        // Build markdown content
        let mut markdown = String::new();

        // Header with session metadata
        markdown.push_str(&format!("# {}\n\n", self.info.title));
        markdown.push_str(&format!("**Session ID**: {}\n", self.info.id));
        markdown.push_str(&format!("**Directory**: {}\n", self.info.directory));
        markdown.push_str(&format!(
            "**Created**: {}\n",
            self.info.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        markdown.push_str(&format!(
            "**Updated**: {}\n",
            self.info.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        markdown.push_str(&format!("**Messages**: {}\n\n", self.info.message_count));
        markdown.push_str("---\n\n");

        // Check if there are any messages
        if self.messages.is_empty() {
            markdown.push_str("*No messages yet*\n");
        } else {
            // Iterate through messages
            for msg in &self.messages {
                match msg.role.as_str() {
                    "user" => {
                        markdown.push_str("## User\n\n");
                        if let Some(content) = &msg.content {
                            markdown.push_str(content);
                            markdown.push_str("\n\n");
                        }
                        markdown.push_str("---\n\n");
                    }
                    "assistant" => {
                        markdown.push_str("## Assistant\n\n");
                        if let Some(content) = &msg.content {
                            markdown.push_str(content);
                            markdown.push_str("\n\n");
                        }

                        // Add tool calls if present
                        if !msg.tool_calls.is_empty() {
                            markdown.push_str("### Tool Calls\n\n");
                            for tool_call in &msg.tool_calls {
                                markdown.push_str(&format!(
                                    "- **{}** (`{}`)\n",
                                    tool_call.name, tool_call.id
                                ));

                                // Format arguments as JSON
                                if let Ok(formatted_args) =
                                    serde_json::to_string_pretty(&tool_call.arguments)
                                {
                                    markdown.push_str("  - Arguments:\n");
                                    markdown.push_str("    ```json\n");
                                    markdown.push_str(&format!(
                                        "    {}\n",
                                        formatted_args.replace("\n", "\n    ")
                                    ));
                                    markdown.push_str("    ```\n");
                                }

                                // Find corresponding result
                                if let Some(result) = msg
                                    .tool_results
                                    .iter()
                                    .find(|r| r.tool_call_id == tool_call.id)
                                {
                                    markdown
                                        .push_str(&format!("  - Result: {}\n", result.observation));
                                    markdown.push_str(&format!("  - Status: {}\n", result.status));
                                } else {
                                    markdown.push_str("  - Result: *No result recorded*\n");
                                }
                                markdown.push_str("\n");
                            }
                        }
                        markdown.push_str("---\n\n");
                    }
                    _ => {}
                }
            }
        }

        // Write to file
        std::fs::write(&file_path, markdown)?;

        // Return the actual filename used
        Ok(file_path.to_string_lossy().to_string())
    }
}

// Helper functions
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn get_id_prefix(id: &str) -> String {
    id.chars().take(8).collect()
}
