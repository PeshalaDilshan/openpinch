use crate::{SandboxBackend, SandboxCommand, SandboxHealth, SandboxOutput, SandboxPaths};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use base64ct::{Base64UrlUnpadded, Encoding};
use openpinch_common::SandboxConfig;
use serde::Serialize;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::process::Command;
use tokio::time::{Duration, sleep, timeout};
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
pub struct LinuxFirecrackerBackend {
    config: SandboxConfig,
    paths: SandboxPaths,
}

impl LinuxFirecrackerBackend {
    pub fn new(config: SandboxConfig, paths: SandboxPaths) -> Result<Self> {
        std::fs::create_dir_all(&paths.workspace_dir).with_context(|| {
            format!(
                "failed to create sandbox workspace {}",
                paths.workspace_dir.display()
            )
        })?;
        Ok(Self { config, paths })
    }

    fn missing_prerequisites(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if which::which(&self.config.firecracker_path).is_err() {
            missing.push(format!(
                "firecracker binary not found: {}",
                self.config.firecracker_path
            ));
        }
        if which::which(&self.config.jailer_path).is_err() {
            missing.push(format!(
                "jailer binary not found: {}",
                self.config.jailer_path
            ));
        }
        if self.config.linux.containerd_in_guest
            && which::which(&self.config.containerd_path).is_err()
        {
            missing.push(format!(
                "containerd binary not found: {}",
                self.config.containerd_path
            ));
        }
        if self.config.kernel_image.is_empty() {
            missing.push("sandbox.kernel_image is not configured".to_owned());
        } else if !PathBuf::from(&self.config.kernel_image).exists() {
            missing.push(format!(
                "kernel image not found: {}",
                self.config.kernel_image
            ));
        }
        if self.config.rootfs_image.is_empty() {
            missing.push("sandbox.rootfs_image is not configured".to_owned());
        } else if !PathBuf::from(&self.config.rootfs_image).exists() {
            missing.push(format!(
                "rootfs image not found: {}",
                self.config.rootfs_image
            ));
        }
        if !self.config.seccomp_profile.is_empty()
            && !PathBuf::from(&self.config.seccomp_profile).exists()
        {
            missing.push(format!(
                "seccomp profile not found: {}",
                self.config.seccomp_profile
            ));
        }

        if nix_like_uid() != 0 {
            missing.push(
                "Linux Firecracker backend expects root or equivalent privileges for jailer"
                    .to_owned(),
            );
        }

        missing
    }
}

#[async_trait]
impl SandboxBackend for LinuxFirecrackerBackend {
    fn name(&self) -> &str {
        "linux-firecracker"
    }

    fn health(&self) -> SandboxHealth {
        SandboxHealth {
            backend: self.name().to_owned(),
            missing_prerequisites: self.missing_prerequisites(),
        }
    }

    async fn execute(&self, command: SandboxCommand) -> Result<SandboxOutput> {
        let missing = self.missing_prerequisites();
        if !missing.is_empty() {
            bail!("sandbox prerequisites missing: {}", missing.join(", "));
        }

        let vm_id = format!("openpinch-{}", Uuid::new_v4().simple());
        let session = TempDir::new_in(&self.paths.workspace_dir)
            .context("failed to create sandbox session directory")?;
        let api_socket = session.path().join("firecracker.socket");
        let chroot_base = session.path().join("jailer");
        let payload = Base64UrlUnpadded::encode_string(
            serde_json::to_string(&GuestPayload::from(command.clone()))
                .context("failed to serialize guest payload")?
                .as_bytes(),
        );

        let child = Command::new(&self.config.jailer_path)
            .arg("--id")
            .arg(&vm_id)
            .arg("--exec-file")
            .arg(&self.config.firecracker_path)
            .arg("--uid")
            .arg("0")
            .arg("--gid")
            .arg("0")
            .arg("--chroot-base-dir")
            .arg(&chroot_base)
            .arg("--")
            .arg("--api-sock")
            .arg(&api_socket)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to spawn jailer")?;

        wait_for_socket(&api_socket).await?;

        let boot_args = format!(
            "console=ttyS0 reboot=k panic=1 pci=off quiet init=/sbin/openpinch-init openpinch_payload_b64={payload}"
        );
        firecracker_request(
            &api_socket,
            "PUT",
            "/machine-config",
            r#"{"vcpu_count":1,"mem_size_mib":512,"ht_enabled":false}"#,
        )?;
        firecracker_request(
            &api_socket,
            "PUT",
            "/boot-source",
            &format!(
                "{{\"kernel_image_path\":\"{}\",\"boot_args\":\"{}\"}}",
                self.config.kernel_image, boot_args
            ),
        )?;
        firecracker_request(
            &api_socket,
            "PUT",
            "/drives/rootfs",
            &format!(
                "{{\"drive_id\":\"rootfs\",\"path_on_host\":\"{}\",\"is_root_device\":true,\"is_read_only\":true}}",
                self.config.rootfs_image
            ),
        )?;
        if !self.config.seccomp_profile.is_empty() {
            firecracker_request(
                &api_socket,
                "PUT",
                "/seccomp",
                &format!("{{\"path\":\"{}\"}}", self.config.seccomp_profile),
            )?;
        }
        firecracker_request(
            &api_socket,
            "PUT",
            "/actions",
            r#"{"action_type":"InstanceStart"}"#,
        )?;

        info!("started Firecracker sandbox session {}", vm_id);

        let output = timeout(Duration::from_secs(90), child.wait_with_output())
            .await
            .context("sandbox execution timed out")?
            .context("failed to wait for jailer/firecracker process")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(SandboxOutput {
            success: output.status.success(),
            exit_code: output.status.code().unwrap_or(-1),
            logs: vec![
                format!("sandbox backend: {}", self.name()),
                format!("vm id: {vm_id}"),
                format!("subject: {}", command.subject),
                format!("capabilities: {}", command.capabilities.join(",")),
                format!(
                    "containerd_in_guest: {}",
                    self.config.linux.containerd_in_guest
                ),
            ],
            stdout,
            stderr,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
struct GuestPayload {
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    allow_network: bool,
    workspace_archive_b64: Option<String>,
}

impl From<SandboxCommand> for GuestPayload {
    fn from(value: SandboxCommand) -> Self {
        Self {
            program: value.program,
            args: value.args,
            env: value.env,
            allow_network: value.allow_network,
            workspace_archive_b64: value.workspace_archive_b64,
        }
    }
}

async fn wait_for_socket(path: &PathBuf) -> Result<()> {
    for _ in 0..100 {
        if path.exists() {
            return Ok(());
        }
        sleep(Duration::from_millis(50)).await;
    }

    bail!(
        "firecracker API socket was not created at {}",
        path.display()
    )
}

fn firecracker_request(socket: &PathBuf, method: &str, path: &str, body: &str) -> Result<()> {
    let mut stream = UnixStream::connect(socket).with_context(|| {
        format!(
            "failed to connect to Firecracker socket {}",
            socket.display()
        )
    })?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(request.as_bytes())
        .context("failed to write Firecracker API request")?;
    stream
        .flush()
        .context("failed to flush Firecracker API request")?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("failed to read Firecracker API response")?;
    if !(response.starts_with("HTTP/1.1 204") || response.starts_with("HTTP/1.1 200")) {
        bail!("Firecracker API call failed for {path}: {response}");
    }

    Ok(())
}

fn nix_like_uid() -> u32 {
    std::fs::metadata(".")
        .map(|metadata| metadata.uid())
        .unwrap_or_default()
}
