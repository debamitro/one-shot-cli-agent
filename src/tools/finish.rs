use super::{Tool, ToolOutput};
use anyhow::Result;
use serde_json::json;

pub struct FinishTool;

impl Tool for FinishTool {
    fn name(&self) -> &str {
        "finish"
    }

    fn description(&self) -> &str {
        "Call this tool when you have completed the user's request and want to provide a final answer. \
         This signals that you are done using tools and ready to conclude. \
         Include your complete response to the user in the 'answer' parameter."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "answer": {
                    "type": "string",
                    "description": "Your final answer or response to the user's request"
                }
            },
            "required": ["answer"]
        })
    }

    fn execute(&self, input: serde_json::Value) -> Result<ToolOutput> {
        let answer = input
            .get("answer")
            .and_then(|v| v.as_str())
            .unwrap_or("Task completed");

        Ok(ToolOutput {
            output: json!({
                "answer": answer,
                "completed": true
            }),
            observation: answer.to_string(),
            display: None,
            status: "success".to_string(),
        })
    }
}
