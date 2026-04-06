use super::ModelProvider;
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use openpinch_common::ModelProviderConfig;
use reqwest::Client;
use serde::Deserialize;

#[derive(Clone)]
pub struct LlamaCppProvider {
    name: String,
    config: ModelProviderConfig,
    client: Client,
}

impl LlamaCppProvider {
    pub fn new(name: String, config: ModelProviderConfig, client: Client) -> Self {
        Self {
            name,
            config,
            client,
        }
    }
}

#[derive(Deserialize)]
struct CompletionResponse {
    content: Option<String>,
    choices: Option<Vec<CompletionChoice>>,
}

#[derive(Deserialize)]
struct CompletionChoice {
    text: Option<String>,
}

#[async_trait]
impl ModelProvider for LlamaCppProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn enabled(&self) -> bool {
        self.config.enabled
    }

    async fn generate(&self, prompt: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/completion", self.config.endpoint))
            .json(&serde_json::json!({
                "prompt": prompt,
                "n_predict": 256,
                "temperature": 0.2
            }))
            .send()
            .await
            .context("llama.cpp completion request failed")?
            .error_for_status()
            .context("llama.cpp returned an error status")?;

        let body = response
            .json::<CompletionResponse>()
            .await
            .context("failed to decode llama.cpp response")?;

        if let Some(content) = body.content {
            return Ok(content);
        }
        if let Some(choice) = body.choices.and_then(|mut choices| choices.pop()) {
            if let Some(text) = choice.text {
                return Ok(text);
            }
        }

        bail!("llama.cpp returned an empty completion body")
    }
}
