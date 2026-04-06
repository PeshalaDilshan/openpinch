use super::ModelProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use openpinch_common::ModelProviderConfig;
use reqwest::Client;
use serde::Deserialize;

#[derive(Clone)]
pub struct OllamaProvider {
    name: String,
    config: ModelProviderConfig,
    client: Client,
}

impl OllamaProvider {
    pub fn new(name: String, config: ModelProviderConfig, client: Client) -> Self {
        Self {
            name,
            config,
            client,
        }
    }
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn enabled(&self) -> bool {
        self.config.enabled
    }

    async fn generate(&self, prompt: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/api/generate", self.config.endpoint))
            .json(&serde_json::json!({
                "model": self.config.model,
                "prompt": prompt,
                "stream": false
            }))
            .send()
            .await
            .context("ollama generate request failed")?
            .error_for_status()
            .context("ollama returned an error status")?;

        let body = response
            .json::<OllamaResponse>()
            .await
            .context("failed to decode ollama response")?;
        Ok(body.response)
    }
}
