use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityMatrix {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub default_capabilities: Vec<String>,
    #[serde(default)]
    pub subjects: BTreeMap<String, CapabilityRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityRule {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub subject: String,
    pub allowed_capabilities: Vec<String>,
    pub denied_capabilities: Vec<String>,
    pub source: String,
}

pub fn load_capability_matrix(path: &Path) -> Result<CapabilityMatrix> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&raw).context("failed to parse capability matrix")
}

impl CapabilityMatrix {
    pub fn evaluate(
        &self,
        subject: &str,
        requested: &[String],
        default_deny: bool,
    ) -> PolicyDecision {
        let mut allowed = Vec::new();
        let mut denied = Vec::new();
        let rule = self
            .subjects
            .iter()
            .find(|(pattern, _)| subject_matches(pattern, subject))
            .map(|(_, rule)| rule);

        for capability in requested {
            let explicitly_denied = rule
                .map(|entry| {
                    entry
                        .deny
                        .iter()
                        .any(|candidate| matches_capability(candidate, capability))
                })
                .unwrap_or(false);
            let explicitly_allowed = rule
                .map(|entry| {
                    entry
                        .allow
                        .iter()
                        .any(|candidate| matches_capability(candidate, capability))
                })
                .unwrap_or(false)
                || self
                    .default_capabilities
                    .iter()
                    .any(|candidate| matches_capability(candidate, capability));

            if explicitly_denied || (!explicitly_allowed && default_deny) {
                denied.push(capability.clone());
            } else {
                allowed.push(capability.clone());
            }
        }

        PolicyDecision {
            subject: subject.to_owned(),
            allowed_capabilities: allowed,
            denied_capabilities: denied,
            source: self.version.clone(),
        }
    }
}

fn subject_matches(pattern: &str, subject: &str) -> bool {
    pattern == subject
        || pattern == "*"
        || pattern
            .strip_suffix('*')
            .map(|prefix| subject.starts_with(prefix))
            .unwrap_or(false)
}

fn matches_capability(pattern: &str, capability: &str) -> bool {
    pattern == capability
        || pattern == "*"
        || pattern
            .strip_suffix('*')
            .map(|prefix| capability.starts_with(prefix))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{CapabilityMatrix, CapabilityRule};
    use std::collections::BTreeMap;

    #[test]
    fn policy_respects_deny_by_default() {
        let matrix = CapabilityMatrix {
            version: "test".to_owned(),
            default_capabilities: vec!["tool.read".to_owned()],
            subjects: BTreeMap::from([(
                "builtin.command".to_owned(),
                CapabilityRule {
                    description: String::new(),
                    allow: vec!["shell.execute".to_owned()],
                    deny: vec!["network.*".to_owned()],
                },
            )]),
        };
        let decision = matrix.evaluate(
            "builtin.command",
            &["shell.execute".to_owned(), "network.egress".to_owned()],
            true,
        );

        assert_eq!(decision.allowed_capabilities, vec!["shell.execute"]);
        assert_eq!(decision.denied_capabilities, vec!["network.egress"]);
    }
}
