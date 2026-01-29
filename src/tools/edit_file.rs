use super::{Tool, ToolOutput};
use anyhow::{Context, Result};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::Path;

pub struct EditFileTool;

impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Create new files or edit existing files with various operations. REQUIRED: Set 'operation' to one of: 'create_file' (new file), 'replace_by_string' (find/replace unique text), 'replace_by_lines' (replace line range), or 'read_file' (view contents)."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["create_file", "replace_by_string", "replace_by_lines", "read_file"],
                    "description": "Operation to perform on the file"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "content": {
                    "type": "string",
                    "description": "Content for create_file or new content for replacements"
                },
                "old_string": {
                    "type": "string",
                    "description": "String to replace (for replace_by_string)"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Starting line number (1-based, for replace_by_lines)"
                },
                "end_line": {
                    "type": "integer",
                    "description": "Ending line number (1-based, for replace_by_lines)"
                }
            },
            "required": ["operation", "file_path"]
        })
    }

    fn execute(&self, input: serde_json::Value) -> Result<ToolOutput> {
        let operation = input
            .get("operation")
            .and_then(|v| v.as_str())
            .context("Missing operation")?;
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .context("Missing file_path")?;

        match operation {
            "create_file" => {
                let content = input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .context("Missing content")?;
                self.create_file(file_path, content)
            }
            "replace_by_string" => {
                let old_string = input
                    .get("old_string")
                    .and_then(|v| v.as_str())
                    .context("Missing old_string")?;
                let new_string = input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .context("Missing content")?;
                self.replace_by_string(file_path, old_string, new_string)
            }
            "replace_by_lines" => {
                let start_line = input
                    .get("start_line")
                    .and_then(|v| v.as_u64())
                    .context("Missing start_line")? as usize;
                let end_line = input
                    .get("end_line")
                    .and_then(|v| v.as_u64())
                    .context("Missing end_line")? as usize;
                let new_content = input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .context("Missing content")?;
                self.replace_by_lines(file_path, start_line, end_line, new_content)
            }
            "read_file" => {
                let start_line = input
                    .get("start_line")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let end_line = input
                    .get("end_line")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                self.read_file(file_path, start_line, end_line)
            }
            _ => Err(anyhow::anyhow!("Unknown operation: {}", operation)),
        }
    }
}

impl EditFileTool {
    fn create_file(&self, file_path: &str, content: &str) -> Result<ToolOutput> {
        if Path::new(file_path).exists() {
            return Err(anyhow::anyhow!("File already exists: {}", file_path));
        }

        if let Some(parent) = Path::new(file_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file =
            fs::File::create(file_path).context(format!("Failed to create file: {}", file_path))?;
        file.write_all(content.as_bytes())?;

        Ok(ToolOutput {
            output: json!({ "file_path": file_path, "lines": content.lines().count() }),
            observation: format!(
                "Created file {} with {} lines",
                file_path,
                content.lines().count()
            ),
            display: Some(format!("✓ Created {}", file_path)),
            status: "success".to_string(),
        })
    }

    fn replace_by_string(
        &self,
        file_path: &str,
        old_string: &str,
        new_string: &str,
    ) -> Result<ToolOutput> {
        let content =
            fs::read_to_string(file_path).context(format!("Failed to read file: {}", file_path))?;

        let occurrences = content.matches(old_string).count();

        if occurrences == 0 {
            return Err(anyhow::anyhow!("String not found in file"));
        }

        if occurrences > 1 {
            return Err(anyhow::anyhow!(
                "String appears {} times, must be unique",
                occurrences
            ));
        }

        let new_content = content.replace(old_string, new_string);

        fs::write(file_path, &new_content)
            .context(format!("Failed to write file: {}", file_path))?;

        Ok(ToolOutput {
            output: json!({ "file_path": file_path, "modified": true }),
            observation: format!("Replaced string in {}", file_path),
            display: Some(format!("✓ Modified {}", file_path)),
            status: "success".to_string(),
        })
    }

    fn replace_by_lines(
        &self,
        file_path: &str,
        start_line: usize,
        end_line: usize,
        new_content: &str,
    ) -> Result<ToolOutput> {
        let content =
            fs::read_to_string(file_path).context(format!("Failed to read file: {}", file_path))?;

        let lines: Vec<&str> = content.lines().collect();

        if start_line < 1 || start_line > lines.len() {
            return Err(anyhow::anyhow!("Invalid start_line: {}", start_line));
        }

        if end_line < start_line || end_line > lines.len() {
            return Err(anyhow::anyhow!("Invalid end_line: {}", end_line));
        }

        let mut new_lines = Vec::new();
        new_lines.extend_from_slice(&lines[..start_line - 1]);
        new_lines.extend(new_content.lines());
        new_lines.extend_from_slice(&lines[end_line..]);

        let new_file_content = new_lines.join("\n") + "\n";

        fs::write(file_path, &new_file_content)
            .context(format!("Failed to write file: {}", file_path))?;

        let old_count = end_line - start_line + 1;
        let new_count = new_content.lines().count();

        Ok(ToolOutput {
            output: json!({
                "file_path": file_path,
                "modified": true,
                "lines_replaced": old_count,
                "new_lines": new_count
            }),
            observation: format!(
                "Replaced lines {}-{} in {}",
                start_line, end_line, file_path
            ),
            display: Some(format!(
                "✓ Modified {} ({} -> {} lines)",
                file_path, old_count, new_count
            )),
            status: "success".to_string(),
        })
    }

    fn read_file(
        &self,
        file_path: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> Result<ToolOutput> {
        let content =
            fs::read_to_string(file_path).context(format!("Failed to read file: {}", file_path))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start = start_line.unwrap_or(1).saturating_sub(1);
        let end = end_line.unwrap_or(total_lines).min(total_lines);

        let selected_lines: Vec<String> = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{}|{}", start + i + 1, line))
            .collect();

        Ok(ToolOutput {
            output: json!({
                "file_path": file_path,
                "total_lines": total_lines,
                "start_line": start + 1,
                "end_line": end,
                "content": selected_lines.join("\n")
            }),
            observation: format!(
                "Read lines {}-{} from {} ({} total lines)",
                start + 1,
                end,
                file_path,
                total_lines
            ),
            display: Some(selected_lines.join("\n")),
            status: "success".to_string(),
        })
    }
}
