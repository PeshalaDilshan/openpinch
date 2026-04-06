use anyhow::{Context, Result, bail};
use base64ct::{Base64, Encoding};
use chrono::Utc;
use openpinch_common::openpinch::SkillInfo;
use openpinch_common::{
    AppConfig, CapabilityMatrix, OpenPinchPaths, PolicyDecision, PolicyReport, RegistryIndex,
    SkillManifest, SkillsConfig, ToolCall, ToolOutcome, bundle_manifest_path,
    install_verified_bundle, load_capability_matrix, read_text_file, verify_registry,
    verify_skill_bundle,
};
use openpinch_sandbox::{SandboxCommand, SandboxHealth, SandboxManager};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tar::Builder;
use tracing::info;

#[derive(Clone)]
pub struct SkillManager {
    config: SkillsConfig,
    paths: OpenPinchPaths,
}

impl SkillManager {
    pub fn new(config: SkillsConfig, paths: OpenPinchPaths) -> Self {
        Self { config, paths }
    }

    pub fn list(&self) -> Result<(Vec<SkillInfo>, String)> {
        let registry = self.registry()?;
        let mut entries = Vec::new();

        for skill in registry.skills {
            let installed_path = self.installed_skill_dir(&skill.id, &skill.version);
            entries.push(SkillInfo {
                id: skill.id,
                version: skill.version,
                name: skill.name,
                description: skill.description,
                installed: installed_path.exists(),
                verified: true,
                entrypoint: skill.entrypoint,
                language: skill.language,
            });
        }

        Ok((entries, registry.version))
    }

    pub fn verify(&self, source: &Path) -> Result<SkillManifest> {
        Ok(verify_skill_bundle(source, &self.resolve(&self.config.trust_root))?.manifest)
    }

    pub fn install(&self, source: &Path, force: bool) -> Result<SkillInfo> {
        let verified = verify_skill_bundle(source, &self.resolve(&self.config.trust_root))?;
        let destination =
            self.installed_skill_dir(&verified.manifest.id, &verified.manifest.version);
        if destination.exists() && !force {
            bail!(
                "skill {}@{} is already installed; re-run with --force to replace it",
                verified.manifest.id,
                verified.manifest.version
            );
        }

        let manifest =
            install_verified_bundle(source, &destination, &self.resolve(&self.config.trust_root))?;

        Ok(SkillInfo {
            id: manifest.id,
            version: manifest.version,
            name: manifest.name,
            description: manifest.description,
            installed: true,
            verified: true,
            entrypoint: manifest.entrypoint,
            language: manifest.language,
        })
    }

    pub fn installed_manifest(&self, skill_id: &str) -> Result<(SkillManifest, PathBuf)> {
        let install_root = self.paths.installs_dir.clone();
        for entry in fs::read_dir(&install_root).with_context(|| {
            format!(
                "failed to read installed skills directory {}",
                install_root.display()
            )
        })? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let manifest_path = bundle_manifest_path(&entry.path());
            if !manifest_path.exists() {
                continue;
            }
            let manifest = serde_json::from_str::<SkillManifest>(&read_text_file(&manifest_path)?)
                .context("failed to decode installed skill manifest")?;
            if manifest.id == skill_id {
                return Ok((manifest, entry.path()));
            }
        }

        bail!("skill {} is not installed", skill_id)
    }

    pub fn packaged_workspace(&self, skill_id: &str) -> Result<(SkillManifest, String)> {
        let (manifest, root) = self.installed_manifest(skill_id)?;
        let mut encoder =
            zstd::Encoder::new(Vec::new(), 3).context("failed to create zstd encoder")?;
        {
            let mut archive = Builder::new(&mut encoder);
            archive
                .append_dir_all(".", &root)
                .with_context(|| format!("failed to archive {}", root.display()))?;
            archive
                .finish()
                .context("failed to finalize skill archive")?;
        }
        let bytes = encoder.finish().context("failed to finish skill archive")?;
        Ok((manifest, Base64::encode_string(&bytes)))
    }

    fn registry(&self) -> Result<RegistryIndex> {
        verify_registry(
            &self.resolve(&self.config.registry_index),
            &self.resolve(&self.config.registry_signature),
            &self.resolve(&self.config.trust_root),
        )
    }

    fn installed_skill_dir(&self, skill_id: &str, version: &str) -> PathBuf {
        self.paths
            .installs_dir
            .join(format!("{skill_id}-{version}"))
    }

    fn resolve(&self, configured: &str) -> PathBuf {
        let path = PathBuf::from(configured);
        if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        }
    }
}

#[derive(Clone)]
pub struct ToolExecutor {
    config: AppConfig,
    _paths: OpenPinchPaths,
    sandbox: SandboxManager,
    skills: SkillManager,
    policy: CapabilityMatrix,
}

impl ToolExecutor {
    pub fn new(
        config: AppConfig,
        paths: OpenPinchPaths,
        sandbox: SandboxManager,
        skills: SkillManager,
    ) -> Result<Self> {
        let policy_path = resolve_path(&config.sandbox.capabilities.matrix_path);
        let policy = load_capability_matrix(&policy_path).with_context(|| {
            format!(
                "failed to load capability matrix from {}",
                policy_path.display()
            )
        })?;

        Ok(Self {
            config,
            _paths: paths,
            sandbox,
            skills,
            policy,
        })
    }

    pub async fn execute(&self, request: ToolCall) -> ToolOutcome {
        match self.execute_inner(request).await {
            Ok(outcome) => outcome,
            Err(error) => ToolOutcome {
                success: false,
                summary: "execution failed".to_owned(),
                data_json: "{}".to_owned(),
                error: error.to_string(),
                logs: vec![],
            },
        }
    }

    pub async fn execute_skill(&self, skill_id: &str, arguments_json: &str) -> ToolOutcome {
        match self.execute_skill_inner(skill_id, arguments_json).await {
            Ok(outcome) => outcome,
            Err(error) => ToolOutcome {
                success: false,
                summary: "skill execution failed".to_owned(),
                data_json: "{}".to_owned(),
                error: error.to_string(),
                logs: vec![],
            },
        }
    }

    pub async fn health(&self) -> SandboxHealth {
        self.sandbox.health()
    }

    pub fn policy_report(&self, subject: &str) -> PolicyReport {
        let requested = vec![
            "shell.execute".to_owned(),
            "tool.local".to_owned(),
            "network.egress".to_owned(),
            "skill.execute".to_owned(),
            "agent.message".to_owned(),
        ];
        let decision = self.policy.evaluate(
            subject,
            &requested,
            self.config.sandbox.capabilities.default_deny,
        );
        PolicyReport {
            subject: decision.subject,
            allowed_capabilities: decision.allowed_capabilities,
            denied_capabilities: decision.denied_capabilities,
            source: decision.source,
        }
    }

    fn authorize(&self, subject: &str, requested: Vec<String>) -> Result<PolicyDecision> {
        let decision = self.policy.evaluate(
            subject,
            &requested,
            self.config.sandbox.capabilities.default_deny,
        );
        if !decision.denied_capabilities.is_empty() {
            bail!(
                "policy denied {} for {}",
                decision.denied_capabilities.join(", "),
                subject
            );
        }
        Ok(decision)
    }

    async fn execute_inner(&self, request: ToolCall) -> Result<ToolOutcome> {
        let arguments = parse_json_object(&request.arguments_json)?;
        match request.target.as_str() {
            "builtin.echo" => {
                self.authorize("builtin.echo", vec!["tool.local".to_owned()])?;
                let message = arguments
                    .get("message")
                    .and_then(Value::as_str)
                    .context("builtin.echo expects arguments.message")?;
                Ok(success_outcome(
                    "echo completed",
                    serde_json::json!({ "message": message }).to_string(),
                    vec!["echo handled locally".to_owned()],
                ))
            }
            "builtin.time" => {
                self.authorize("builtin.time", vec!["tool.local".to_owned()])?;
                Ok(success_outcome(
                    "current time resolved",
                    serde_json::json!({ "utc": Utc::now().to_rfc3339() }).to_string(),
                    vec!["timestamp generated locally".to_owned()],
                ))
            }
            "builtin.system-info" => {
                self.authorize("builtin.system-info", vec!["tool.inspect".to_owned()])?;
                Ok(success_outcome(
                    "system info collected",
                    serde_json::json!({
                        "os": std::env::consts::OS,
                        "arch": std::env::consts::ARCH,
                    })
                    .to_string(),
                    vec!["system information collected".to_owned()],
                ))
            }
            "builtin.command" => {
                let mut requested = vec!["shell.execute".to_owned()];
                if request.allow_network {
                    requested.push("network.egress".to_owned());
                }
                let decision = self.authorize("builtin.command", requested.clone())?;

                let program = arguments
                    .get("program")
                    .and_then(Value::as_str)
                    .context("builtin.command expects arguments.program")?;
                let args = arguments
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let sandbox_output = self
                    .sandbox
                    .execute(SandboxCommand {
                        subject: "builtin.command".to_owned(),
                        capabilities: decision.allowed_capabilities,
                        program: program.to_owned(),
                        args,
                        env: vec![],
                        allow_network: request.allow_network,
                        workspace_archive_b64: None,
                    })
                    .await?;

                Ok(ToolOutcome {
                    success: sandbox_output.success,
                    summary: "sandbox command finished".to_owned(),
                    data_json: serde_json::json!({
                        "stdout": sandbox_output.stdout,
                        "stderr": sandbox_output.stderr,
                        "exit_code": sandbox_output.exit_code,
                    })
                    .to_string(),
                    error: String::new(),
                    logs: sandbox_output.logs,
                })
            }
            target if target.starts_with("skill.") => {
                let skill_id = target.trim_start_matches("skill.");
                self.execute_skill_inner(skill_id, &request.arguments_json)
                    .await
            }
            other => bail!("unsupported tool target {}", other),
        }
    }

    async fn execute_skill_inner(
        &self,
        skill_id: &str,
        arguments_json: &str,
    ) -> Result<ToolOutcome> {
        let subject = format!("skill.{skill_id}");
        let decision = self.authorize(
            &subject,
            vec!["skill.execute".to_owned(), "sandbox.execute".to_owned()],
        )?;
        let (manifest, workspace_archive_b64) = self.skills.packaged_workspace(skill_id)?;
        let command = match manifest.language.as_str() {
            "shell" => SandboxCommand {
                subject: subject.clone(),
                capabilities: decision.allowed_capabilities.clone(),
                program: "/bin/sh".to_owned(),
                args: vec![format!("/workspace/{}", manifest.entrypoint)],
                env: vec![(
                    "OPENPINCH_SKILL_ARGS_JSON".to_owned(),
                    arguments_json.to_owned(),
                )],
                allow_network: false,
                workspace_archive_b64: Some(workspace_archive_b64),
            },
            _ => SandboxCommand {
                subject: subject.clone(),
                capabilities: decision.allowed_capabilities.clone(),
                program: format!("/workspace/{}", manifest.entrypoint),
                args: vec![],
                env: vec![(
                    "OPENPINCH_SKILL_ARGS_JSON".to_owned(),
                    arguments_json.to_owned(),
                )],
                allow_network: false,
                workspace_archive_b64: Some(workspace_archive_b64),
            },
        };

        let output = self.sandbox.execute(command).await?;
        info!(
            "skill {} completed with exit {}",
            skill_id, output.exit_code
        );
        Ok(ToolOutcome {
            success: output.success,
            summary: format!("skill {} executed", skill_id),
            data_json: serde_json::json!({
                "stdout": output.stdout,
                "stderr": output.stderr,
                "exit_code": output.exit_code,
            })
            .to_string(),
            error: String::new(),
            logs: output.logs,
        })
    }
}

fn resolve_path(configured: &str) -> PathBuf {
    let path = PathBuf::from(configured);
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn parse_json_object(raw: &str) -> Result<Value> {
    let value = serde_json::from_str::<Value>(raw).context("failed to parse JSON arguments")?;
    if !value.is_object() {
        bail!("tool arguments must be a JSON object");
    }
    Ok(value)
}

fn success_outcome(summary: &str, data_json: String, logs: Vec<String>) -> ToolOutcome {
    ToolOutcome {
        success: true,
        summary: summary.to_owned(),
        data_json,
        error: String::new(),
        logs,
    }
}
