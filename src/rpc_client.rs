use crate::error::{CliError, CliResult};
use colored::Colorize;
use reqwest::Client;
use serde_json::{json, Value};

pub struct RpcClient {
    client: Client,
    urls: Vec<String>,
    commitment: String,
}

impl RpcClient {
    pub fn new(url: String, fallback_urls: Vec<String>, commitment: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let mut urls = vec![url];
        urls.extend(fallback_urls);

        Self {
            client,
            urls,
            commitment,
        }
    }

    pub async fn call(
        &self,
        method: &str,
        params: Value,
    ) -> CliResult<Value> {
        let mut last_err: Option<CliError> = None;
        let safe_urls: Vec<String> = self.urls.iter().map(|u| crate::strip_api_key(u)).collect();

        eprintln!(
            "  {} {} → {}",
            "→".dimmed(),
            method.cyan(),
            safe_urls[0].dimmed(),
        );

        for (i, url) in self.urls.iter().enumerate() {
            match self.call_url(url, method, &params).await {
                Ok(result) => {
                    if i > 0 {
                        eprintln!(
                            "  {} Succeeded on fallback #{}",
                            "✓".green().bold(),
                            i,
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    if i + 1 < self.urls.len() {
                        eprintln!(
                            "  {} {} failed: {}",
                            "✗".red().bold(),
                            safe_urls[i],
                            e,
                        );
                        eprintln!();
                        eprintln!(
                            "  {} Falling back to {}",
                            "→".dimmed(),
                            safe_urls[i + 1].dimmed(),
                        );
                    } else {
                        eprintln!(
                            "  {} {} failed: {}",
                            "✗".red().bold(),
                            safe_urls[i],
                            e,
                        );
                    }
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| "No RPC URLs configured".into()))
    }

    async fn call_url(
        &self,
        url: &str,
        method: &str,
        params: &Value,
    ) -> CliResult<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let resp = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status, text.trim()).into());
        }

        let parsed: Value = serde_json::from_str(&text)?;

        if let Some(error) = parsed.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown RPC error");
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            return Err(format!("RPC error {}: {}", code, msg).into());
        }

        Ok(parsed["result"].clone())
    }

    pub fn commitment(&self) -> &str {
        &self.commitment
    }
}
