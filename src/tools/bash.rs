use super::{Tool, ToolOutput};
use anyhow::{Context, Result};
use serde_json::json;
use std::process::{Command, Stdio};

pub struct BashTool;

impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute shell commands in the system. Provide the full command string in the 'command' parameter. Optionally specify 'cwd' to set the working directory."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory for the command (optional)"
                }
            },
            "required": ["command"]
        })
    }

    fn execute(&self, input: serde_json::Value) -> Result<ToolOutput> {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .context("Missing command")?;
        let cwd = input.get("cwd").and_then(|v| v.as_str());

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        if let Some(working_dir) = cwd {
            cmd.current_dir(working_dir);
        }

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute command")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();

        let observation = if success {
            if stdout.is_empty() && stderr.is_empty() {
                "Command executed successfully (no output)".to_string()
            } else {
                format!("Command executed successfully")
            }
        } else {
            format!(
                "Command failed with exit code: {}",
                output.status.code().unwrap_or(-1)
            )
        };

        let display = if !stdout.is_empty() {
            stdout.clone()
        } else if !stderr.is_empty() {
            stderr.clone()
        } else {
            String::new()
        };

        Ok(ToolOutput {
            output: json!({
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": output.status.code().unwrap_or(-1),
                "success": success
            }),
            observation,
            display: Some(display),
            status: if success { "success" } else { "error" }.to_string(),
        })
    }
}
