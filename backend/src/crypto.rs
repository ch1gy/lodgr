/// AES-256-GCM encryption and Argon2id key derivation.
/// This is the only module in the codebase that touches raw cryptography.
/// No other module may import aes_gcm directly.
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::{rngs::OsRng, RngCore};
use std::sync::Arc;

use crate::error::{AppError, AppResult};

/// A heap-allocated 32-byte encryption key wrapped in an Arc so it can be
/// cheaply cloned into Axum state without copying the key bytes.
pub type EncryptionKey = Arc<[u8; 32]>;

/// Derive a 32-byte AES key from `passphrase` and a hex-encoded `salt`.
///
/// Parameters follow OWASP recommendations for interactive authentication:
/// memory = 64 MiB, iterations = 3, parallelism = 1.
/// This function is called ONCE at startup; the passphrase and salt are not
/// retained after this call returns.
pub fn derive_key(passphrase: &str, salt_hex: &str) -> anyhow::Result<[u8; 32]> {
    let salt = decode_hex(salt_hex)
        .map_err(|e| anyhow::anyhow!("ENCRYPTION_SALT is not valid hex: {e}"))?;

    if salt.len() < 16 {
        anyhow::bail!(
            "ENCRYPTION_SALT must be at least 16 bytes (32 hex chars), got {} bytes",
            salt.len()
        );
    }

    // 64 MiB memory, 3 iterations, 1-thread parallelism, 32-byte output.
    let params =
        Params::new(65_536, 3, 1, Some(32)).map_err(|e| anyhow::anyhow!("argon2 params: {e}"))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), &salt, &mut key)
        .map_err(|e| anyhow::anyhow!("key derivation failed: {e}"))?;

    Ok(key)
}

/// Encrypt `plaintext` with AES-256-GCM using the supplied key.
///
/// Returns `(nonce_hex, ciphertext_hex)`. A fresh 12-byte nonce is generated
/// for every call via `OsRng`.
pub fn encrypt(key: &[u8; 32], plaintext: &str) -> AppResult<(String, String)> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| AppError::Internal("encryption failed".into()))?;

    Ok((hex_encode(&nonce_bytes), hex_encode(&ciphertext)))
}

/// Hash an email address for blind-index lookup using Argon2id with a fixed
/// system-wide salt. Same algorithm and parameters as `derive_key` so an
/// attacker must run the full memory-hard computation per guess.
///
/// The result is a 64-character hex string stored in `users.email_hash`.
/// `salt_hex` must be the `EMAIL_HASH_SALT` env value — never change it after
/// the first user is written, as all login lookups depend on it.
pub fn hash_email(email: &str, salt_hex: &str) -> AppResult<String> {
    let raw =
        derive_key(email, salt_hex).map_err(|e| AppError::Internal(format!("email hash: {e}")))?;
    Ok(hex_encode(&raw))
}

/// Decrypt `ciphertext_hex` using `key` and `nonce_hex`.
///
/// Returns the plaintext string, or `AppError::Internal` if decryption fails
/// (wrong key, corrupted data, or invalid hex).
pub fn decrypt(key: &[u8; 32], nonce_hex: &str, ciphertext_hex: &str) -> AppResult<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    let nonce_bytes =
        decode_hex(nonce_hex).map_err(|_| AppError::Internal("invalid nonce hex".into()))?;
    let ciphertext = decode_hex(ciphertext_hex)
        .map_err(|_| AppError::Internal("invalid ciphertext hex".into()))?;

    if nonce_bytes.len() != 12 {
        return Err(AppError::Internal(format!(
            "nonce must be 12 bytes, got {}",
            nonce_bytes.len()
        )));
    }

    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext_bytes = cipher.decrypt(nonce, ciphertext.as_ref()).map_err(|_| {
        AppError::Internal("decryption failed — data corrupted or wrong key".into())
    })?;

    String::from_utf8(plaintext_bytes)
        .map_err(|_| AppError::Internal("decrypted data is not valid UTF-8".into()))
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err(format!("odd hex length: {}", s.len()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| format!("invalid hex at position {i}: {e}"))
        })
        .collect()
}
