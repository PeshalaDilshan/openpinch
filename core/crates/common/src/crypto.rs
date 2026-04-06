use anyhow::{Context, Result, bail};
use base64ct::{Base64, Encoding};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBlob {
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Clone)]
pub struct SessionKeypair {
    secret: StaticSecret,
    public: PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIdentity {
    pub public_key: String,
    pub fingerprint: String,
}

impl SessionKeypair {
    pub fn generate() -> Self {
        let mut seed = [0_u8; 32];
        rand::thread_rng().fill_bytes(&mut seed);
        let secret = StaticSecret::from(seed);
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }

    pub fn identity(&self) -> SessionIdentity {
        let public_bytes = self.public.as_bytes();
        SessionIdentity {
            public_key: Base64::encode_string(public_bytes),
            fingerprint: format!("{:x}", Sha256::digest(public_bytes)),
        }
    }

    pub fn shared_key(&self, peer_public_b64: &str) -> Result<[u8; 32]> {
        let bytes = Base64::decode_vec(peer_public_b64).context("invalid peer public key")?;
        let public_bytes: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .context("peer public key must be 32 bytes")?;
        let public = PublicKey::from(public_bytes);
        Ok(derive_shared_key(&self.secret, &public))
    }
}

pub fn generate_data_key() -> [u8; 32] {
    let mut key = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

pub fn derive_key_from_material(material: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(material);
    let mut key = [0_u8; 32];
    key.copy_from_slice(&digest[..32]);
    key
}

pub fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> Result<EncryptedBlob> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce_bytes = [0_u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| anyhow::anyhow!("encryption failed"))?;
    Ok(EncryptedBlob {
        nonce: Base64::encode_string(&nonce_bytes),
        ciphertext: Base64::encode_string(&ciphertext),
    })
}

pub fn decrypt_bytes(key: &[u8; 32], blob: &EncryptedBlob) -> Result<Vec<u8>> {
    let nonce = Base64::decode_vec(&blob.nonce).context("invalid blob nonce")?;
    let ciphertext = Base64::decode_vec(&blob.ciphertext).context("invalid blob ciphertext")?;
    if nonce.len() != 24 {
        bail!("XChaCha20 nonce must be 24 bytes");
    }

    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(XNonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| anyhow::anyhow!("decryption failed"))
}

pub fn derive_shared_key(secret: &StaticSecret, public: &PublicKey) -> [u8; 32] {
    let shared = secret.diffie_hellman(public);
    derive_key_from_material(shared.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{SessionKeypair, decrypt_bytes, encrypt_bytes, generate_data_key};

    #[test]
    fn encrypt_roundtrip() {
        let key = generate_data_key();
        let blob = encrypt_bytes(&key, b"openpinch").expect("encrypt");
        let decrypted = decrypt_bytes(&key, &blob).expect("decrypt");
        assert_eq!(decrypted, b"openpinch");
    }

    #[test]
    fn shared_key_matches() {
        let a = SessionKeypair::generate();
        let b = SessionKeypair::generate();
        let shared_a = a
            .shared_key(&b.identity().public_key)
            .expect("shared key from a");
        let shared_b = b
            .shared_key(&a.identity().public_key)
            .expect("shared key from b");
        assert_eq!(shared_a, shared_b);
    }
}
