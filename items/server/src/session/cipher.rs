//! Encrypting a session's tokens at rest.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;

const NONCE_LEN: usize = 12;

/// Encrypts and decrypts tokens with AES-256-GCM. Each ciphertext is bound to
/// the session it belongs to, so it cannot be moved to another row.
#[derive(Clone)]
pub struct SessionCipher {
    cipher: Aes256Gcm,
}

#[derive(Debug, thiserror::Error)]
#[error("the session key must be 32 bytes, base64-encoded")]
pub struct InvalidKey;

#[derive(Debug, thiserror::Error)]
#[error("a stored token could not be decrypted")]
pub struct Undecryptable;

impl std::fmt::Debug for SessionCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionCipher(<key>)")
    }
}

impl SessionCipher {
    /// A cipher under `key`, 32 bytes in standard base64.
    pub fn from_base64(key: &str) -> Result<Self, InvalidKey> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(key.trim())
            .map_err(|_| InvalidKey)?;
        let cipher = Aes256Gcm::new_from_slice(&bytes).map_err(|_| InvalidKey)?;
        Ok(Self { cipher })
    }

    /// Encrypts `token` for the session `id_hash`: a random nonce, then the
    /// ciphertext and tag.
    ///
    /// # Panics
    ///
    /// If the system has no randomness.
    // r[impl session.storage]
    #[must_use]
    pub fn seal(&self, id_hash: &[u8], token: &str) -> Vec<u8> {
        let mut nonce = [0; NONCE_LEN];
        getrandom::fill(&mut nonce).expect("system randomness");
        let ciphertext = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: token.as_bytes(),
                    aad: id_hash,
                },
            )
            .expect("AES-GCM encrypts any message this short");
        let mut sealed = nonce.to_vec();
        sealed.extend(ciphertext);
        sealed
    }

    /// Decrypts what [`seal`](Self::seal) produced for `id_hash`.
    pub fn open(&self, id_hash: &[u8], sealed: &[u8]) -> Result<String, Undecryptable> {
        if sealed.len() < NONCE_LEN {
            return Err(Undecryptable);
        }
        let (nonce, ciphertext) = sealed.split_at(NONCE_LEN);
        let plain = self
            .cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: id_hash,
                },
            )
            .map_err(|_| Undecryptable)?;
        String::from_utf8(plain).map_err(|_| Undecryptable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

    // r[verify session.storage]
    #[test]
    fn opens_only_what_it_sealed_for_the_same_session() {
        let cipher = SessionCipher::from_base64(KEY).unwrap();
        let sealed = cipher.seal(b"session-a", "a-token");
        assert!(!sealed.windows(7).any(|w| w == b"a-token"));
        assert_eq!(cipher.open(b"session-a", &sealed).unwrap(), "a-token");
        assert!(cipher.open(b"session-b", &sealed).is_err());
    }

    #[test]
    fn refuses_a_key_of_the_wrong_length() {
        assert!(SessionCipher::from_base64("AAEC").is_err());
        assert!(SessionCipher::from_base64("not base64!").is_err());
    }
}
