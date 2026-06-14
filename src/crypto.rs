//! AES-256-CBC + PBKDF2-HMAC-SHA1 (100 iterations) token encryption,
//! used by provider-B's request signing path.

use crate::error::AppError;
use crate::secret_endpoints::OBFUSCATED_PASSWORD;
use aes::cipher::{BlockEncryptMut, KeyIvInit};
use base64::Engine;
use cbc::Encryptor;
use hmac::Hmac;
use pbkdf2::pbkdf2;
use rand::{rngs::SysRng, TryRng};
use sha1::Sha1;

type Aes256CbcEnc = Encryptor<aes::Aes256>;

/// Encrypt `plaintext` under a fresh random salt and IV and return the
/// ciphertext as a hex-encoded, then base64-encoded, single string.
///
/// The output layout is: `base64( hex( salt(32) || iv(16) || ciphertext ) )`.
///
/// # Errors
///
/// - [`AppError::Crypto`] when PBKDF2, the cipher init, or the
///   ciphertext padding fails.
///
/// # Examples
///
/// ```
/// use youtube_legend_cli::crypto::encrypt_token;
///
/// let token = encrypt_token("https://youtu.be/abc;;12345").unwrap();
/// assert!(!token.is_empty());
/// ```
pub fn encrypt_token(plaintext: &str) -> Result<String, AppError> {
    let mut salt = [0u8; 32];
    SysRng
        .try_fill_bytes(&mut salt)
        .map_err(|e| AppError::Crypto(format!("system rng failed: {e}")))?;
    let mut iv = [0u8; 16];
    SysRng
        .try_fill_bytes(&mut iv)
        .map_err(|e| AppError::Crypto(format!("system rng failed: {e}")))?;

    let mut key = [0u8; 32];
    pbkdf2::<Hmac<Sha1>>(OBFUSCATED_PASSWORD, &salt, 100, &mut key)
        .map_err(|e| AppError::Crypto(format!("pbkdf2 failed: {e}")))?;

    let cipher = Aes256CbcEnc::new_from_slices(&key, &iv)
        .map_err(|e| AppError::Crypto(format!("cipher init failed: {e}")))?;

    let msg_len = plaintext.len();
    let mut buf = vec![0u8; msg_len + 16];
    buf[..msg_len].copy_from_slice(plaintext.as_bytes());
    let ciphertext = cipher
        .encrypt_padded_mut::<cbc::cipher::block_padding::Pkcs7>(&mut buf, msg_len)
        .map_err(|e| AppError::Crypto(format!("encrypt failed: {e:?}")))?;

    let mut combined = Vec::with_capacity(48 + ciphertext.len());
    combined.extend_from_slice(&salt);
    combined.extend_from_slice(&iv);
    combined.extend_from_slice(ciphertext);

    let hex_combined = hex::encode(&combined);
    Ok(base64::engine::general_purpose::STANDARD.encode(hex_combined.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_non_empty_token() {
        let token = encrypt_token("https://youtu.be/abc;;12345").unwrap();
        assert!(!token.is_empty());
    }

    #[test]
    fn produces_different_tokens_for_same_input() {
        let t1 = encrypt_token("test message").unwrap();
        let t2 = encrypt_token("test message").unwrap();
        assert_ne!(t1, t2, "salt and iv must be unique per request");
    }

    #[test]
    fn produces_base64() {
        let token = encrypt_token("hello").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&token)
            .unwrap();
        let hex_str = String::from_utf8(decoded).unwrap();
        let bytes = hex::decode(&hex_str).unwrap();
        assert!(bytes.len() >= 48);
    }

    #[test]
    fn obfuscated_password_is_resolved_from_constants() {
        assert!(!OBFUSCATED_PASSWORD.is_empty());
        assert!(OBFUSCATED_PASSWORD.len() >= 16);
    }
}
