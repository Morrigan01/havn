use std::sync::Arc;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

use crate::registry::Registry;

/// Global project_id — shared secrets not scoped to any project.
pub const GLOBAL: i64 = 0;

pub struct SecretStore {
    master_key: [u8; 32],
    registry: Arc<Registry>,
}

impl SecretStore {
    pub fn new(registry: Arc<Registry>) -> Self {
        let master_key = load_or_create_master_key();
        Self { master_key, registry }
    }

    pub fn set(&self, project_id: i64, key: &str, value: &str) {
        let (nonce, ciphertext) = encrypt(&self.master_key, value);
        self.registry.set_secret(project_id, key, &nonce, &ciphertext);
    }

    pub fn get(&self, project_id: i64, key: &str) -> Option<String> {
        let (nonce, ciphertext) = self.registry.get_secret(project_id, key)?;
        decrypt(&self.master_key, &nonce, &ciphertext)
    }

    pub fn list(&self, project_id: i64) -> Vec<String> {
        self.registry.list_secret_keys(project_id)
    }

    pub fn delete(&self, project_id: i64, key: &str) -> bool {
        self.registry.delete_secret(project_id, key)
    }
}

fn load_or_create_master_key() -> [u8; 32] {
    let key_path = crate::config::config_dir().join("master.key");

    if key_path.exists() {
        if let Ok(bytes) = std::fs::read(&key_path) {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return key;
            }
        }
    }

    // Generate a new random master key.
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);

    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&key_path, key).ok();

    // Restrict to owner-read-only (Unix only).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).ok();
    }

    key
}

fn encrypt(key: &[u8; 32], plaintext: &str) -> (Vec<u8>, Vec<u8>) {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .expect("AES-GCM encryption failed");
    (nonce_bytes.to_vec(), ciphertext)
}

fn decrypt(key: &[u8; 32], nonce_bytes: &[u8], ciphertext: &[u8]) -> Option<String> {
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext).ok()?;
    String::from_utf8(plaintext).ok()
}
