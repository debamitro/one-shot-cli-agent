use super::{Tool, ToolOutput};
use anyhow::{Context, Result};
use serde_json::json;
use std::io::{self, Write};
use std::process::{Command, Stdio};

pub struct BashTool;

impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute shell commands in the system. Provide the full command string in the 'command' parameter. Optionally specify 'cwd' to set the working directory. Set 'skip_approval' to true for read-only commands to skip user confirmation."
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
                },
                "skip_approval": {
                    "type": "boolean",
                    "description": "Skip user approval prompt. Use true ONLY for read-only commands (e.g., git status, ls, ps). Default: false",
                    "default": false
                },
                "description": {
                    "type": "string",
                    "description": "Human-readable description of what the command does (optional, used in approval prompt)"
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
        let skip_approval = input
            .get("skip_approval")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let description = input.get("description").and_then(|v| v.as_str());

        // Approval mechanism
        if !skip_approval {
            println!("\n🔍 Bash Command Approval Required:");
            if let Some(desc) = description {
                println!("   Description: {}", desc);
            }
            println!("   Command: {}", command);
            if let Some(working_dir) = cwd {
                println!("   Working directory: {}", working_dir);
            }
            print!("\nExecute this command? [y/N]: ");
            io::stdout().flush()?;

            let mut input_line = String::new();
            io::stdin().read_line(&mut input_line)?;
            let approved = matches!(input_line.trim().to_lowercase().as_str(), "y" | "yes");

            if !approved {
                return Ok(ToolOutput {
                    output: json!({
                        "approved": false,
                        "command": command
                    }),
                    observation: "Command execution cancelled by user".to_string(),
                    display: Some("User declined to execute the command".to_string()),
                    status: "cancelled".to_string(),
                });
            }
        }

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
