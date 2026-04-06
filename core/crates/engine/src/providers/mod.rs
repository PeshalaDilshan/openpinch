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
    providers: BTreeMap<String, Box<dyn ModelProvider>>,
    configs: BTreeMap<String, ModelProviderConfig>,
    provider_order: Vec<String>,
}

impl ProviderRegistry {
    pub fn from_config(config: &BTreeMap<String, ModelProviderConfig>) -> Self {
        let client = Client::new();
        let mut providers: BTreeMap<String, Box<dyn ModelProvider>> = BTreeMap::new();

        for (name, provider) in config {
            match provider.kind.as_str() {
                "ollama" => {
                    providers.insert(
                        name.clone(),
                        Box::new(OllamaProvider::new(
                            name.clone(),
                            provider.clone(),
                            client.clone(),
                        )),
                    );
                }
                "llamacpp" => {
                    providers.insert(
                        name.clone(),
                        Box::new(LlamaCppProvider::new(
                            name.clone(),
                            provider.clone(),
                            client.clone(),
                        )),
                    );
                }
                "openai-compatible" => {
                    providers.insert(
                        name.clone(),
                        Box::new(OpenAiCompatProvider::new(
                            name.clone(),
                            provider.clone(),
                            client.clone(),
                        )),
                    );
                }
                _ => {}
            }
        }

        let provider_order = config.keys().cloned().collect::<Vec<_>>();
        Self {
            providers,
            configs: config.clone(),
            provider_order,
        }
    }

    pub fn enabled_names(&self) -> Vec<String> {
        self.provider_order
            .iter()
            .filter_map(|name| self.providers.get(name))
            .filter(|provider| provider.enabled())
            .map(|provider| provider.name().to_owned())
            .collect()
    }

    pub fn config_for(&self, name: &str) -> Option<&ModelProviderConfig> {
        self.configs.get(name)
    }

    pub async fn generate_with_provider(&self, provider: &str, prompt: &str) -> Result<String> {
        let engine = self
            .providers
            .get(provider)
            .ok_or_else(|| anyhow::anyhow!("unknown provider {provider}"))?;
        if !engine.enabled() {
            bail!("provider {provider} is disabled");
        }
        engine.generate(prompt).await
    }

    pub async fn generate_with_routing(
        &self,
        order: &[String],
        prompt: &str,
    ) -> Result<(String, String)> {
        let mut errors = Vec::new();
        for name in order {
            let Some(provider) = self.providers.get(name) else {
                continue;
            };
            if !provider.enabled() {
                continue;
            }
            match provider.generate(prompt).await {
                Ok(response) => return Ok((name.clone(), response)),
                Err(error) => errors.push(format!("{}: {}", provider.name(), error)),
            }
        }

        if errors.is_empty() {
            bail!("no enabled local model providers are configured");
        }
        bail!("no local model backend succeeded ({})", errors.join("; "))
    }
}
