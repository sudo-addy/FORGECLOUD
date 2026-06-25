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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngExt;

    fn get_random_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        rand::rng().fill(&mut key);
        key
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = get_random_key();
        let plaintext = b"Hello, ForgeCloud!";

        let encrypted = encrypt_chunk(plaintext, &key).unwrap();
        assert_ne!(plaintext.as_slice(), encrypted.as_slice());

        let decrypted = decrypt_chunk(&encrypted, &key).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_random_nonce_generation() {
        let key = get_random_key();
        let plaintext = b"Identical plaintext should produce different ciphertexts";

        let encrypted1 = encrypt_chunk(plaintext, &key).unwrap();
        let encrypted2 = encrypt_chunk(plaintext, &key).unwrap();

        // The outputs should be entirely different due to random nonce
        assert_ne!(encrypted1, encrypted2);

        // Nonces specifically should differ (first 12 bytes)
        assert_ne!(&encrypted1[0..NONCE_LEN], &encrypted2[0..NONCE_LEN]);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = get_random_key();
        let key2 = get_random_key();
        let plaintext = b"Top secret data";

        let encrypted = encrypt_chunk(plaintext, &key1).unwrap();

        // Decrypting with key2 should fail
        let result = decrypt_chunk(&encrypted, &key2);
        assert!(result.is_err());
    }

    #[test]
    fn test_modified_ciphertext_fails() {
        let key = get_random_key();
        let plaintext = b"Data integrity test";

        let mut encrypted = encrypt_chunk(plaintext, &key).unwrap();

        // Modify a byte in the ciphertext part (after the nonce)
        let last_idx = encrypted.len() - 1;
        encrypted[last_idx] ^= 1;

        let result = decrypt_chunk(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_modified_nonce_fails() {
        let key = get_random_key();
        let plaintext = b"Nonce integrity test";

        let mut encrypted = encrypt_chunk(plaintext, &key).unwrap();

        // Modify a byte in the nonce part
        encrypted[0] ^= 1;

        let result = decrypt_chunk(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_data_too_short_fails() {
        let key = get_random_key();
        // 11 bytes is less than the 12-byte nonce requirement
        let too_short = vec![0u8; NONCE_LEN - 1];

        let result = decrypt_chunk(&too_short, &key);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Encrypted data is too short to contain a nonce"
        );
    }

    #[test]
    fn test_empty_file() {
        let key = get_random_key();
        let plaintext = b"";

        let encrypted = encrypt_chunk(plaintext, &key).unwrap();
        let decrypted = decrypt_chunk(&encrypted, &key).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_one_byte_file() {
        let key = get_random_key();
        let plaintext = b"x";

        let encrypted = encrypt_chunk(plaintext, &key).unwrap();
        let decrypted = decrypt_chunk(&encrypted, &key).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_large_buffer() {
        let key = get_random_key();
        // Simulating a large chunk (1MB)
        let size_to_test = 1024 * 1024;
        let mut plaintext = vec![0u8; size_to_test];
        rand::rng().fill(plaintext.as_mut_slice());

        let encrypted = encrypt_chunk(&plaintext, &key).unwrap();
        let decrypted = decrypt_chunk(&encrypted, &key).unwrap();
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_utf8_text() {
        let key = get_random_key();
        let plaintext = "Hello 🌍, this is a UTF-8 test for ForgeCloud! 🚀".as_bytes();

        let encrypted = encrypt_chunk(plaintext, &key).unwrap();
        let decrypted = decrypt_chunk(&encrypted, &key).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
        let decoded_str = String::from_utf8(decrypted).unwrap();
        assert_eq!(
            decoded_str,
            "Hello 🌍, this is a UTF-8 test for ForgeCloud! 🚀"
        );
    }

    #[test]
    fn test_truncated_ciphertext_fails() {
        let key = get_random_key();
        let plaintext = b"This data will be truncated";

        let mut encrypted = encrypt_chunk(plaintext, &key).unwrap();

        // Remove the last byte (which is part of the auth tag or ciphertext)
        encrypted.pop();

        let result = decrypt_chunk(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_corrupted_authentication_tag_fails() {
        let key = get_random_key();
        let plaintext = b"Corrupted tag test";

        let mut encrypted = encrypt_chunk(plaintext, &key).unwrap();

        // GCM auth tag is the last 16 bytes. Flip a bit in the last byte.
        let last_idx = encrypted.len() - 1;
        encrypted[last_idx] ^= 0b10101010;

        let result = decrypt_chunk(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_repeated_encryption_produces_unique_ciphertexts() {
        let key = get_random_key();
        let plaintext = b"Repeated encryption data";

        let mut ciphertexts = std::collections::HashSet::new();

        for _ in 0..100 {
            let encrypted = encrypt_chunk(plaintext, &key).unwrap();

            // Decrypt to verify correctness
            let decrypted = decrypt_chunk(&encrypted, &key).unwrap();
            assert_eq!(plaintext.as_slice(), decrypted.as_slice());

            // Insert returns false if the value was already present
            let is_new = ciphertexts.insert(encrypted);
            assert!(
                is_new,
                "Ciphertext collision detected! Nonce reuse occurred."
            );
        }
    }

    #[tokio::test]
    async fn test_concurrent_encryption_decryption() {
        use std::sync::Arc;

        let key = Arc::new(get_random_key());
        let plaintext = Arc::new(b"Concurrent stress test payload".to_vec());

        let mut handles = Vec::new();

        for _ in 0..100 {
            let key_clone = key.clone();
            let plaintext_clone = plaintext.clone();

            let handle = tokio::spawn(async move {
                let encrypted = encrypt_chunk(&plaintext_clone, &key_clone).unwrap();
                let decrypted = decrypt_chunk(&encrypted, &key_clone).unwrap();
                assert_eq!(*plaintext_clone, decrypted);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }
    }
}
