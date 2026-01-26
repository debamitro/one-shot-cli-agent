use super::{LLMProvider, Message, StreamChunk, ToolCall};
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<Delta>,
    content_block: Option<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(rename = "type")]
    delta_type: String,
    text: Option<String>,
    partial_json: Option<String>,
}

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
        }
    }

    fn convert_messages(&self, messages: Vec<Message>) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system = None;
        let mut converted = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => system = Some(msg.content),
                _ => converted.push(AnthropicMessage {
                    role: msg.role,
                    content: msg.content,
                }),
            }
        }

        (system, converted)
    }

    fn convert_tools(&self, tools: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
        tools
            .into_iter()
            .filter_map(|tool| {
                let name = tool.get("name")?.as_str()?;
                let description = tool.get("description")?.as_str()?;
                let input_schema = tool.get("input_schema")?;

                Some(json!({
                    "name": name,
                    "description": description,
                    "input_schema": input_schema
                }))
            })
            .collect()
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<StreamChunk> {
        let (system, converted_messages) = self.convert_messages(messages);
        
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 8192,
            messages: converted_messages,
            tools: tools.map(|t| self.convert_tools(t)),
            system,
            stream: false,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send Anthropic request")?;

        let anthropic_response: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        let mut content = None;
        let mut tool_calls = Vec::new();

        for block in anthropic_response.content {
            match block {
                ContentBlock::Text { text } => content = Some(text),
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments: input,
                    });
                }
            }
        }

        Ok(StreamChunk {
            content,
            tool_calls,
            finished: true,
        })
    }

    async fn stream_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamChunk>> {
        let (system, converted_messages) = self.convert_messages(messages);
        
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 8192,
            messages: converted_messages,
            tools: tools.map(|t| self.convert_tools(t)),
            system,
            stream: true,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send Anthropic stream request")?;

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            use futures::StreamExt;
            
            // State for accumulating tool call parameters
            let mut current_tool_call: Option<(String, String, String)> = None; // (id, name, accumulated_json)

            while let Some(chunk_result) = stream.next().await {
                if let Ok(chunk) = chunk_result {
                    let text = String::from_utf8_lossy(&chunk);
                    
                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                                match event.event_type.as_str() {
                                    "content_block_delta" => {
                                        if let Some(delta) = event.delta {
                                            if let Some(text) = delta.text {
                                                let chunk = StreamChunk {
                                                    content: Some(text),
                                                    tool_calls: Vec::new(),
                                                    finished: false,
                                                };
                                                if tx.send(chunk).await.is_err() {
                                                    return;
                                                }
                                            }
                                            
                                            // Accumulate tool input JSON deltas
                                            if delta.delta_type == "input_json_delta" {
                                                if let Some(partial_json) = delta.partial_json {
                                                    if let Some((_, _, ref mut json)) = current_tool_call {
                                                        json.push_str(&partial_json);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    "content_block_start" => {
                                        if let Some(ContentBlock::ToolUse { id, name, input: _ }) =
                                            event.content_block
                                        {
                                            // Start accumulating - initial input is always empty {}
                                            current_tool_call = Some((id, name, String::new()));
                                        }
                                    }
                                    "content_block_stop" => {
                                        // Finalize accumulated tool call
                                        if let Some((id, name, json_str)) = current_tool_call.take() {
                                            let arguments = if json_str.is_empty() {
                                                serde_json::json!({})
                                            } else {
                                                serde_json::from_str(&json_str).unwrap_or_else(|_| serde_json::json!({}))
                                            };
                                            
                                            let chunk = StreamChunk {
                                                content: None,
                                                tool_calls: vec![ToolCall {
                                                    id,
                                                    name,
                                                    arguments,
                                                }],
                                                finished: false,
                                            };
                                            if tx.send(chunk).await.is_err() {
                                                return;
                                            }
                                        }
                                    }
                                    "message_stop" => {
                                        let chunk = StreamChunk {
                                            content: None,
                                            tool_calls: Vec::new(),
                                            finished: true,
                                        };
                                        let _ = tx.send(chunk).await;
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            
            // Send final finished chunk if stream ended without message_stop
            let _ = tx.send(StreamChunk {
                content: None,
                tool_calls: Vec::new(),
                finished: true,
            }).await;
        });

        Ok(rx)
    }
}
