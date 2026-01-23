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
    pub fn new(title: String, directory: String, storage_path: PathBuf) -> Self {
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

    pub fn add_assistant_message(&mut self, content: Option<String>, tool_calls: Vec<ToolCall>) -> String {
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

    pub fn add_tool_result(&mut self, tool_call_id: String, output: serde_json::Value, observation: String, status: String) {
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
}
