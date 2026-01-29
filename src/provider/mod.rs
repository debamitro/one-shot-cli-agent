pub mod anthropic;
pub mod openai;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finished: bool,
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<StreamChunk>;

    async fn stream_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamChunk>>;
}
