mod llamacpp;
mod ollama;
mod openai_compat;

use anyhow::{Result, bail};
use async_trait::async_trait;
use openpinch_common::ModelProviderConfig;
use reqwest::Client;
use std::collections::BTreeMap;

pub use llamacpp::LlamaCppProvider;
pub use ollama::OllamaProvider;
pub use openai_compat::OpenAiCompatProvider;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn name(&self) -> &str;
    fn enabled(&self) -> bool;
    async fn generate(&self, prompt: &str) -> Result<String>;
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn ModelProvider>>,
}

impl ProviderRegistry {
    pub fn from_config(config: &BTreeMap<String, ModelProviderConfig>) -> Self {
        let client = Client::new();
        let mut providers: Vec<Box<dyn ModelProvider>> = Vec::new();

        for (name, provider) in config {
            match provider.kind.as_str() {
                "ollama" => providers.push(Box::new(OllamaProvider::new(
                    name.clone(),
                    provider.clone(),
                    client.clone(),
                ))),
                "llamacpp" => providers.push(Box::new(LlamaCppProvider::new(
                    name.clone(),
                    provider.clone(),
                    client.clone(),
                ))),
                "openai-compatible" => providers.push(Box::new(OpenAiCompatProvider::new(
                    name.clone(),
                    provider.clone(),
                    client.clone(),
                ))),
                _ => {}
            }
        }

        Self { providers }
    }

    pub fn enabled_names(&self) -> Vec<String> {
        self.providers
            .iter()
            .filter(|provider| provider.enabled())
            .map(|provider| provider.name().to_owned())
            .collect()
    }

    pub async fn generate(&self, prompt: &str) -> Result<String> {
        let mut errors = Vec::new();

        for provider in &self.providers {
            if !provider.enabled() {
                continue;
            }

            match provider.generate(prompt).await {
                Ok(response) => return Ok(response),
                Err(error) => errors.push(format!("{}: {}", provider.name(), error)),
            }
        }

        bail!("no local model backend succeeded ({})", errors.join("; "))
    }
}
