use super::{Tool, ToolOutput};
use anyhow::{Context, Result};
use serde_json::json;
use std::process::{Command, Stdio};

pub struct FileSearchTool;

impl Tool for FileSearchTool {
    fn name(&self) -> &str {
        "file_search"
    }

    fn description(&self) -> &str {
        "Search for files using glob patterns or grep for content in files using ripgrep. REQUIRED: Set 'operation' to 'glob' for filename pattern matching (e.g., '**/*.rs'), or 'grep' for content search using regex patterns."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["glob", "grep"],
                    "description": "Operation to perform: glob for filename patterns, grep for content search"
                },
                "pattern": {
                    "type": "string",
                    "description": "Pattern to search for (glob pattern or regex)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)"
                },
                "file_type": {
                    "type": "string",
                    "description": "File type filter for grep (e.g., 'rs', 'py', 'js')"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Case sensitive search (default: true)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return"
                }
            },
            "required": ["operation", "pattern"]
        })
    }

    fn execute(&self, input: serde_json::Value) -> Result<ToolOutput> {
        let operation = input
            .get("operation")
            .and_then(|v| v.as_str())
            .context("Missing operation")?;
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .context("Missing pattern")?;
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        match operation {
            "glob" => self.glob(pattern, path),
            "grep" => {
                let file_type = input.get("file_type").and_then(|v| v.as_str());
                let case_sensitive = input
                    .get("case_sensitive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let max_results = input
                    .get("max_results")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                
                self.grep(pattern, path, file_type, case_sensitive, max_results)
            }
            _ => Err(anyhow::anyhow!("Unknown operation: {}", operation)),
        }
    }
}

impl FileSearchTool {
    fn glob(&self, pattern: &str, path: &str) -> Result<ToolOutput> {
        let glob_pattern = if path == "." {
            pattern.to_string()
        } else {
            format!("{}/{}", path.trim_end_matches('/'), pattern)
        };

        let paths: Vec<String> = glob::glob(&glob_pattern)
            .context("Invalid glob pattern")?
            .filter_map(|entry| entry.ok())
            .filter_map(|p| p.to_str().map(String::from))
            .collect();

        let count = paths.len();
        let observation = if count == 0 {
            "No files found matching the pattern".to_string()
        } else {
            format!("Found {} file(s)", count)
        };

        Ok(ToolOutput {
            output: json!({ "files": paths }),
            observation,
            display: Some(paths.join("\n")),
            status: "success".to_string(),
        })
    }

    fn grep(
        &self,
        pattern: &str,
        path: &str,
        file_type: Option<&str>,
        case_sensitive: bool,
        max_results: Option<usize>,
    ) -> Result<ToolOutput> {
        let mut cmd = Command::new("rg");
        cmd.arg("--json")
            .arg("--no-heading")
            .arg(pattern)
            .arg(path);

        if !case_sensitive {
            cmd.arg("-i");
        }

        if let Some(ft) = file_type {
            cmd.arg("-t").arg(ft);
        }

        if let Some(max) = max_results {
            cmd.arg("--max-count").arg(max.to_string());
        }

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute ripgrep")?;

        if !output.status.success() && output.stdout.is_empty() {
            return Ok(ToolOutput {
                output: json!({ "matches": [] }),
                observation: "No matches found".to_string(),
                display: None,
                status: "success".to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut matches = Vec::new();
        let mut files = std::collections::HashSet::new();

        for line in stdout.lines() {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(line) {
                if json_val.get("type").and_then(|t| t.as_str()) == Some("match") {
                    let data = &json_val["data"];
                    let file = data["path"]["text"].as_str().unwrap_or("");
                    let line_num = data["line_number"].as_u64().unwrap_or(0);
                    let text = data["lines"]["text"].as_str().unwrap_or("");

                    files.insert(file.to_string());
                    matches.push(json!({
                        "file": file,
                        "line": line_num,
                        "text": text.trim()
                    }));
                }
            }
        }

        let observation = format!(
            "Found {} match(es) in {} file(s)",
            matches.len(),
            files.len()
        );

        let display = matches
            .iter()
            .take(20)
            .map(|m| {
                format!(
                    "{}:{}: {}",
                    m["file"].as_str().unwrap_or(""),
                    m["line"],
                    m["text"].as_str().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput {
            output: json!({ "matches": matches }),
            observation,
            display: Some(display),
            status: "success".to_string(),
        })
    }
}
