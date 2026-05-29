use backend::crypto;
use std::sync::Arc;

// ── encrypt / decrypt ─────────────────────────────────────────────────────────

#[test]
fn encrypt_then_decrypt_returns_original_plaintext() {
    let key = Arc::new([1u8; 32]);
    let plaintext = "hello, world";
    let (nonce, ciphertext) = crypto::encrypt(&key, plaintext).unwrap();
    let recovered = crypto::decrypt(&key, &nonce, &ciphertext).unwrap();
    assert_eq!(recovered, plaintext);
}

#[test]
fn decrypt_with_wrong_key_returns_error() {
    let key = Arc::new([1u8; 32]);
    let wrong_key = Arc::new([2u8; 32]);
    let (nonce, ciphertext) = crypto::encrypt(&key, "secret").unwrap();
    assert!(crypto::decrypt(&wrong_key, &nonce, &ciphertext).is_err());
}

#[test]
fn decrypt_with_wrong_nonce_returns_error() {
    let key = Arc::new([1u8; 32]);
    let (_, ciphertext) = crypto::encrypt(&key, "secret").unwrap();
    let bad_nonce = "000000000000000000000000"; // valid hex len, wrong value
    assert!(crypto::decrypt(&key, bad_nonce, &ciphertext).is_err());
}

#[test]
fn two_encryptions_of_same_plaintext_produce_different_ciphertext() {
    let key = Arc::new([1u8; 32]);
    let (nonce1, ct1) = crypto::encrypt(&key, "hello").unwrap();
    let (nonce2, ct2) = crypto::encrypt(&key, "hello").unwrap();
    // Each call generates a fresh random nonce — outputs must differ.
    assert_ne!(nonce1, nonce2);
    assert_ne!(ct1, ct2);
}

#[test]
fn hex_decode_fails_on_odd_length_input() {
    let key = Arc::new([1u8; 32]);
    // A nonce of odd hex length triggers the is_multiple_of(2) guard in decode_hex.
    let odd_nonce = "abc"; // 3 chars — odd length
    assert!(crypto::decrypt(&key, odd_nonce, "deadbeef").is_err());
}
