use super::{LLMProvider, Message, StreamChunk, ToolCall};
use anyhow::{Context, Result};
use async_openai::{
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, ChatCompletionTool, ChatCompletionToolType,
        CreateChatCompletionRequestArgs, FunctionObjectArgs,
    },
    Client,
};
use async_trait::async_trait;
use futures::StreamExt;

pub struct OpenAIProvider {
    client: Client<async_openai::config::OpenAIConfig>,
    model: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        let mut config = async_openai::config::OpenAIConfig::new().with_api_key(api_key);
        
        if let Some(url) = base_url {
            config = config.with_api_base(url);
        }
        
        let client = Client::with_config(config);
        
        Self {
            client,
            model: model.unwrap_or_else(|| "gpt-4o".to_string()),
        }
    }

    fn convert_messages(&self, messages: Vec<Message>) -> Vec<ChatCompletionRequestMessage> {
        messages
            .into_iter()
            .map(|msg| match msg.role.as_str() {
                "system" => ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(msg.content)
                        .build()
                        .unwrap(),
                ),
                "user" => ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content)
                        .build()
                        .unwrap(),
                ),
                _ => ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content)
                        .build()
                        .unwrap(),
                ),
            })
            .collect()
    }

    fn convert_tools(&self, tools: Vec<serde_json::Value>) -> Vec<ChatCompletionTool> {
        tools
            .into_iter()
            .filter_map(|tool| {
                let name = tool.get("name")?.as_str()?;
                let description = tool.get("description")?.as_str()?.to_string();
                let parameters = tool.get("input_schema")?.clone();

                Some(ChatCompletionTool {
                    r#type: ChatCompletionToolType::Function,
                    function: FunctionObjectArgs::default()
                        .name(name)
                        .description(description)
                        .parameters(parameters)
                        .build()
                        .ok()?,
                })
            })
            .collect()
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<StreamChunk> {
        let converted_messages = self.convert_messages(messages);
        
        let mut request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(converted_messages)
            .to_owned();

        if let Some(tool_defs) = tools {
            let converted_tools = self.convert_tools(tool_defs);
            if !converted_tools.is_empty() {
                request = request.tools(converted_tools).to_owned();
            }
        }

        let request = request.build()?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .context("Failed to get OpenAI completion")?;

        let choice = response
            .choices
            .first()
            .context("No choices in response")?;

        let content = choice.message.content.clone();
        let tool_calls = choice
            .message
            .tool_calls
            .as_ref()
            .map(|calls| {
                calls
                    .iter()
                    .map(|call| ToolCall {
                        id: call.id.clone(),
                        name: call.function.name.clone(),
                        arguments: serde_json::from_str(&call.function.arguments)
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();

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
        let converted_messages = self.convert_messages(messages);
        
        let mut request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(converted_messages)
            .to_owned();

        if let Some(tool_defs) = tools {
            let converted_tools = self.convert_tools(tool_defs);
            if !converted_tools.is_empty() {
                request = request.tools(converted_tools).to_owned();
            }
        }

        let request = request.build()?;

        let mut stream = self
            .client
            .chat()
            .create_stream(request)
            .await
            .context("Failed to create OpenAI stream")?;

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        if let Some(choice) = response.choices.first() {
                            let content = choice.delta.content.clone();
                            let tool_calls = choice
                                .delta
                                .tool_calls
                                .as_ref()
                                .map(|calls| {
                                    calls
                                        .iter()
                                        .filter_map(|call| {
                                            Some(ToolCall {
                                                id: call.id.clone()?,
                                                name: call.function.as_ref()?.name.clone()?,
                                                arguments: serde_json::from_str(
                                                    &call.function.as_ref()?.arguments.clone()?,
                                                )
                                                .unwrap_or_default(),
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();

                            let finished = choice.finish_reason.is_some();

                            let chunk = StreamChunk {
                                content,
                                tool_calls,
                                finished,
                            };

                            if tx.send(chunk).await.is_err() {
                                break;
                            }
                            
                            if finished {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            
            // Send final finished chunk if stream ended without finish_reason
            let _ = tx.send(StreamChunk {
                content: None,
                tool_calls: Vec::new(),
                finished: true,
            }).await;
        });

        Ok(rx)
    }
}
