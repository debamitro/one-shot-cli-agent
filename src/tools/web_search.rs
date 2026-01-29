use super::{Tool, ToolOutput};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub struct WebSearchTool {
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerperResponse {
    #[serde(rename = "organic")]
    organic: Option<Vec<OrganicResult>>,
    #[serde(rename = "answerBox")]
    answer_box: Option<serde_json::Value>,
    #[serde(rename = "knowledgeGraph")]
    knowledge_graph: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OrganicResult {
    title: String,
    link: String,
    snippet: String,
    #[serde(default)]
    position: i32,
}

impl WebSearchTool {
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }

    async fn search(
        &self,
        query: &str,
        num_results: usize,
        search_type: &str,
    ) -> Result<SerperResponse> {
        let api_key = self
            .api_key
            .as_ref()
            .context("SERPER_API_KEY not set. Get a free API key from https://serper.dev")?;

        let client = reqwest::Client::new();
        let endpoint = match search_type {
            "news" => "https://google.serper.dev/news",
            "images" => "https://google.serper.dev/images",
            _ => "https://google.serper.dev/search",
        };

        let body = json!({
            "q": query,
            "num": num_results
        });

        let response = client
            .post(endpoint)
            .header("X-API-KEY", api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Serper API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Serper API request failed ({}): {}", status, error_text);
        }

        let result = response
            .json::<SerperResponse>()
            .await
            .context("Failed to parse Serper API response")?;

        Ok(result)
    }
}

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using Google search. Returns top results with titles, snippets, and URLs. Supports web, news, and image search."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5, max: 10)"
                },
                "search_type": {
                    "type": "string",
                    "enum": ["web", "news", "images"],
                    "description": "Type of search (default: web)"
                }
            },
            "required": ["query"]
        })
    }

    fn execute(&self, input: serde_json::Value) -> Result<ToolOutput> {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .context("Missing query")?;
        let num_results = input
            .get("num_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .min(10) as usize;
        let search_type = input
            .get("search_type")
            .and_then(|v| v.as_str())
            .unwrap_or("web");

        // Use block_in_place to allow blocking on async code from within async context
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.search(query, num_results, search_type))
        })?;

        let results = result.organic.unwrap_or_default();
        let count = results.len();

        let observation = if count == 0 {
            format!("No results found for '{}'", query)
        } else {
            format!("Found {} result(s) for '{}'", count, query)
        };

        let display = results
            .iter()
            .map(|r| {
                format!(
                    "{}. {}\n   {}\n   {}",
                    r.position, r.title, r.link, r.snippet
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(ToolOutput {
            output: json!({
                "results": results,
                "query": query,
                "total_results": count
            }),
            observation,
            display: Some(display),
            status: "success".to_string(),
        })
    }
}
