use super::{Tool, ToolOutput};
use anyhow::{Context, Result};
use serde_json::json;
use websearch::{providers::duckduckgo::DuckDuckGoProvider, SearchOptions};

pub struct WebSearchDDGTool;

impl WebSearchDDGTool {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        search_type: &str,
    ) -> Result<Vec<websearch::types::SearchResult>> {
        let provider: Box<dyn websearch::types::SearchProvider> = match search_type {
            "images" => Box::new(DuckDuckGoProvider::for_images()),
            "news" => Box::new(DuckDuckGoProvider::for_news()),
            _ => Box::new(DuckDuckGoProvider::new()),
        };

        let options = SearchOptions {
            query: query.to_string(),
            max_results: Some(num_results as u32),
            provider,
            ..Default::default()
        };

        let results = websearch::web_search(options)
            .await
            .context("DuckDuckGo search failed")?;

        Ok(results)
    }
}

impl Tool for WebSearchDDGTool {
    fn name(&self) -> &str {
        "web_search_ddg"
    }

    fn description(&self) -> &str {
        "Search the web using DuckDuckGo (free, no API key required). Returns search results with titles, snippets, and URLs. Supports web, news, and image search."
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

        let results = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.search(query, num_results, search_type))
        })?;

        let count = results.len();

        let observation = if count == 0 {
            format!("No results found for '{}'", query)
        } else {
            format!("Found {} result(s) for '{}' (via DuckDuckGo)", count, query)
        };

        let display = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "{}. {}\n   {}\n   {}",
                    i + 1,
                    r.title,
                    r.url,
                    r.snippet.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(ToolOutput {
            output: json!({
                "results": results,
                "query": query,
                "total_results": count,
                "provider": "duckduckgo"
            }),
            observation,
            display: Some(display),
            status: "success".to_string(),
        })
    }
}
