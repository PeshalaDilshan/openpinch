use anyhow::{Context, Result, bail};
use base64ct::{Base64, Encoding};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::TempDir;
use walkdir::WalkDir;
use zstd::Decoder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustRoot {
    pub version: String,
    pub keys: Vec<TrustedKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedKey {
    pub key_id: String,
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureEnvelope {
    pub key_id: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub entrypoint: String,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub version: String,
    pub skills: Vec<RegistrySkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySkill {
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub entrypoint: String,
    pub source: String,
}

#[derive(Debug)]
pub struct VerifiedSkillBundle {
    pub manifest: SkillManifest,
    pub unpacked_dir: TempDir,
}

pub fn load_trust_root(path: &Path) -> Result<TrustRoot> {
    let raw = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(serde_json::from_slice(&raw).context("failed to parse trust root")?)
}

pub fn verify_registry(
    index_path: &Path,
    signature_path: &Path,
    root_path: &Path,
) -> Result<RegistryIndex> {
    let root = load_trust_root(root_path)?;
    let payload =
        fs::read(index_path).with_context(|| format!("failed to read {}", index_path.display()))?;
    let signature = read_signature(signature_path)?;
    verify_bytes(&payload, &signature, &root)?;
    Ok(serde_json::from_slice(&payload).context("failed to parse registry index")?)
}

pub fn verify_skill_bundle(path: &Path, root_path: &Path) -> Result<VerifiedSkillBundle> {
    let unpacked_dir = TempDir::new().context("failed to create temporary skill directory")?;
    if path.is_dir() {
        copy_directory(path, unpacked_dir.path())?;
    } else {
        unpack_skill_archive(path, unpacked_dir.path())?;
    }

    let manifest_path = unpacked_dir.path().join("manifest.json");
    let signature_path = unpacked_dir.path().join("manifest.sig.json");
    let manifest_bytes = fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let signature = read_signature(&signature_path)?;
    let root = load_trust_root(root_path)?;
    verify_bytes(&manifest_bytes, &signature, &root)?;

    let manifest = serde_json::from_slice::<SkillManifest>(&manifest_bytes)
        .context("failed to parse skill manifest")?;
    verify_manifest_files(unpacked_dir.path(), &manifest)?;

    Ok(VerifiedSkillBundle {
        manifest,
        unpacked_dir,
    })
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let data = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut digest = Sha256::new();
    digest.update(data);
    Ok(format!("{:x}", digest.finalize()))
}

fn unpack_skill_archive(path: &Path, output_dir: &Path) -> Result<()> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let decoder = Decoder::new(file).context("failed to decode zstd stream")?;
    let mut archive = Archive::new(decoder);
    archive
        .unpack(output_dir)
        .with_context(|| format!("failed to unpack {}", path.display()))
}

fn copy_directory(from: &Path, to: &Path) -> Result<()> {
    for entry in WalkDir::new(from) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(from)?;
        let target = to.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)
                .with_context(|| format!("failed to create {}", target.display()))?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    entry.path().display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

fn read_signature(path: &Path) -> Result<SignatureEnvelope> {
    let raw = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(serde_json::from_slice(&raw).context("failed to parse signature envelope")?)
}

fn verify_bytes(payload: &[u8], signature: &SignatureEnvelope, root: &TrustRoot) -> Result<()> {
    let key = root
        .keys
        .iter()
        .find(|candidate| candidate.key_id == signature.key_id)
        .context("signature key not trusted")?;
    let public_key_bytes =
        Base64::decode_vec(&key.public_key).context("invalid base64 public key")?;
    let verifying_key = VerifyingKey::from_bytes(
        public_key_bytes
            .as_slice()
            .try_into()
            .context("public key must be 32 bytes")?,
    )
    .context("invalid Ed25519 public key")?;

    let signature_bytes =
        Base64::decode_vec(&signature.signature).context("invalid base64 signature")?;
    let signature = Signature::from_bytes(
        signature_bytes
            .as_slice()
            .try_into()
            .context("signature must be 64 bytes")?,
    );

    verifying_key
        .verify(payload, &signature)
        .context("signature verification failed")
}

fn verify_manifest_files(root: &Path, manifest: &SkillManifest) -> Result<()> {
    for file in &manifest.files {
        let path = root.join(&file.path);
        if !path.exists() {
            bail!("manifest referenced missing file {}", file.path);
        }
        let actual = sha256_file(&path)?;
        if actual != file.sha256 {
            bail!(
                "digest mismatch for {}: expected {}, got {}",
                file.path,
                file.sha256,
                actual
            );
        }
    }
    Ok(())
}

pub fn read_text_file(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(buffer)
}

pub fn install_verified_bundle(
    source: &Path,
    destination: &Path,
    root_path: &Path,
) -> Result<SkillManifest> {
    let verified = verify_skill_bundle(source, root_path)?;
    if destination.exists() {
        fs::remove_dir_all(destination)
            .with_context(|| format!("failed to replace {}", destination.display()))?;
    }
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    copy_directory(verified.unpacked_dir.path(), destination)?;
    Ok(verified.manifest)
}

pub fn bundle_manifest_path(bundle_root: &Path) -> PathBuf {
    bundle_root.join("manifest.json")
}
