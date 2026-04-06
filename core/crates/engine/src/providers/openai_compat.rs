use super::ModelProvider;
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use openpinch_common::ModelProviderConfig;
use reqwest::Client;
use serde::Deserialize;

#[derive(Clone)]
pub struct OpenAiCompatProvider {
    name: String,
    config: ModelProviderConfig,
    client: Client,
}

impl OpenAiCompatProvider {
    pub fn new(name: String, config: ModelProviderConfig, client: Client) -> Self {
        Self {
            name,
            config,
            client,
        }
    }
}

#[derive(Deserialize)]
struct ChatCompletion {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

#[async_trait]
impl ModelProvider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn enabled(&self) -> bool {
        self.config.enabled
    }

    async fn generate(&self, prompt: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.endpoint))
            .json(&serde_json::json!({
                "model": self.config.model,
                "messages": [{"role": "user", "content": prompt}],
                "temperature": 0.2
            }))
            .send()
            .await
            .context("OpenAI-compatible completion request failed")?
            .error_for_status()
            .context("OpenAI-compatible backend returned an error status")?;

        let body = response
            .json::<ChatCompletion>()
            .await
            .context("failed to decode OpenAI-compatible response")?;
        let choice = body
            .choices
            .into_iter()
            .next()
            .context("backend returned no choices")?;
        if choice.message.content.is_empty() {
            bail!("backend returned empty content");
        }
        Ok(choice.message.content)
    }
}
