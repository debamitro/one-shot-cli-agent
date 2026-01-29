use super::{Tool, ToolOutput};
use anyhow::{Context, Result};
use serde_json::json;

pub struct URLFetchTool;

impl URLFetchTool {
    async fn fetch_url(&self, url: &str, format: &str, max_length: usize) -> Result<String> {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; CodeAgent/1.0)")
            .build()?;

        let response = client
            .get(url)
            .send()
            .await
            .context("Failed to fetch URL")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP request failed with status: {}", response.status());
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let text = response
            .text()
            .await
            .context("Failed to read response body")?;

        let processed = if content_type.contains("text/html") {
            match format {
                "text" => html2text::from_read(text.as_bytes(), 80),
                "markdown" => {
                    let document = scraper::Html::parse_document(&text);
                    let selector = scraper::Selector::parse("body").unwrap();
                    let body = document
                        .select(&selector)
                        .next()
                        .map(|el| el.text().collect::<Vec<_>>().join(" "))
                        .unwrap_or_else(|| text.clone());

                    html2text::from_read(body.as_bytes(), 80)
                }
                _ => text,
            }
        } else {
            text
        };

        let truncated = if processed.len() > max_length {
            format!(
                "{}...\n\n[Content truncated: {} chars total, showing first {} chars]",
                &processed[..max_length],
                processed.len(),
                max_length
            )
        } else {
            processed
        };

        Ok(truncated)
    }
}

impl Tool for URLFetchTool {
    fn name(&self) -> &str {
        "url_fetch"
    }

    fn description(&self) -> &str {
        "Fetch and extract content from web pages. Automatically converts HTML to readable text or markdown format. Useful for reading documentation, articles, and web content."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch content from"
                },
                "format": {
                    "type": "string",
                    "enum": ["text", "markdown", "raw"],
                    "description": "Output format (default: text)"
                },
                "max_length": {
                    "type": "integer",
                    "description": "Maximum content length in characters (default: 10000)"
                }
            },
            "required": ["url"]
        })
    }

    fn execute(&self, input: serde_json::Value) -> Result<ToolOutput> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .context("Missing url")?;
        let format = input
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");
        let max_length = input
            .get("max_length")
            .and_then(|v| v.as_u64())
            .unwrap_or(10000) as usize;

        let content = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.fetch_url(url, format, max_length))
        })?;

        let observation = format!("Fetched content from {} ({} format)", url, format);

        Ok(ToolOutput {
            output: json!({
                "url": url,
                "content": content,
                "format": format
            }),
            observation,
            display: Some(content),
            status: "success".to_string(),
        })
    }
}
