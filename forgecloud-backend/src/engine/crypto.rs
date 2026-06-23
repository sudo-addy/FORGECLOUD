use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use rand::RngExt;

const NONCE_LEN: usize = 12;

/// Encrypts a plaintext chunk using AES-256-GCM.
/// A unique 12-byte nonce is generated and prepended to the ciphertext.
pub fn encrypt_chunk(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypts an encrypted chunk using AES-256-GCM.
/// Expects the first 12 bytes to be the nonce.
pub fn decrypt_chunk(encrypted_data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if encrypted_data.len() < NONCE_LEN {
        return Err(anyhow!("Encrypted data is too short to contain a nonce"));
    }

    let (nonce_bytes, ciphertext) = encrypted_data.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new(key.into());

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("Decryption failed: {}", e))?;

    Ok(plaintext)
}
