//! The client's DPoP key (RFC 9449, ES256), and the proof it signs for each
//! request.

use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::signature::Signer as _;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::json;

use crate::error::CliError;

/// A P-256 key that signs DPoP proofs.
pub struct DpopKey(SigningKey);

impl DpopKey {
    /// A new random key.
    #[must_use]
    pub fn generate() -> Self {
        loop {
            let mut scalar = [0; 32];
            getrandom::fill(&mut scalar).expect("system randomness");
            // All but a vanishing few 32-byte strings are valid scalars.
            if let Ok(key) = SigningKey::from_slice(&scalar) {
                return Self(key);
            }
        }
    }

    /// The key saved as [`DpopKey::export`] wrote it.
    pub fn import(saved: &str) -> Result<Self, CliError> {
        URL_SAFE_NO_PAD
            .decode(saved)
            .ok()
            .and_then(|bytes| SigningKey::from_slice(&bytes).ok())
            .map(Self)
            .ok_or_else(|| {
                CliError::not_signed_in("The saved DPoP key can't be read; run `kuria login` again")
            })
    }

    /// The private scalar, for the secret store only.
    #[must_use]
    pub fn export(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.to_bytes())
    }

    fn coordinates(&self) -> (Vec<u8>, Vec<u8>) {
        let point = self.0.verifying_key().to_sec1_point(false);
        let bytes = point.as_bytes();
        (bytes[1..33].to_vec(), bytes[33..65].to_vec())
    }

    /// The key's thumbprint, as `dpop_jkt` and `cnf.jkt` name it.
    #[must_use]
    pub fn thumbprint(&self) -> String {
        let (x, y) = self.coordinates();
        konklav_token::ec_thumbprint(&x, &y)
    }

    /// A proof for `method` at `url` (its query and fragment left out),
    /// bound to `access_token` if given, with the server's `nonce` if it
    /// asked for one.
    // r[impl cli.dpop]
    #[must_use]
    pub fn proof(
        &self,
        method: &str,
        url: &url::Url,
        access_token: Option<&str>,
        nonce: Option<&str>,
    ) -> String {
        let (x, y) = self.coordinates();
        let header = json!({
            "typ": "dpop+jwt",
            "alg": "ES256",
            "jwk": { "kty": "EC", "crv": "P-256", "x": URL_SAFE_NO_PAD.encode(x), "y": URL_SAFE_NO_PAD.encode(y) },
        });
        let mut htu = url.clone();
        htu.set_query(None);
        htu.set_fragment(None);
        let mut jti = [0; 16];
        getrandom::fill(&mut jti).expect("system randomness");
        let mut claims = json!({
            "jti": URL_SAFE_NO_PAD.encode(jti),
            "htm": method,
            "htu": htu.as_str(),
            "iat": now(),
        });
        if let Some(token) = access_token {
            claims["ath"] = konklav_token::access_token_hash(token).into();
        }
        if let Some(nonce) = nonce {
            claims["nonce"] = nonce.into();
        }
        let input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let signature: Signature = self.0.sign(input.as_bytes());
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
    }
}

/// Now, in Unix seconds.
#[must_use]
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use konklav_token::{ProofRequest, verify_proof};

    fn request<'a>(url: &'a str, token: Option<&'a str>, jkt: &'a str) -> ProofRequest<'a> {
        ProofRequest {
            method: "POST",
            url,
            access_token: token,
            bound_to: Some(jkt),
            now: now(),
            max_age: 60,
            leeway: 5,
        }
    }

    // r[verify cli.dpop]
    #[test]
    fn signs_proofs_konklav_accepts() {
        let key = DpopKey::generate();
        let jkt = key.thumbprint();
        let url = url::Url::parse("https://kuria.example/graphql?x=1").unwrap();
        let proof = key.proof("POST", &url, Some("the-token"), None);
        let verified = verify_proof(
            &proof,
            &request("https://kuria.example/graphql", Some("the-token"), &jkt),
        )
        .unwrap();
        assert_eq!(verified.jkt, jkt);
        // Bound to its token and its URL.
        assert!(
            verify_proof(
                &proof,
                &request("https://kuria.example/graphql", Some("another"), &jkt)
            )
            .is_err()
        );
        assert!(
            verify_proof(
                &proof,
                &request("https://kuria.example/other", Some("the-token"), &jkt)
            )
            .is_err()
        );
        // Fresh each time.
        let again = key.proof("POST", &url, Some("the-token"), None);
        assert_ne!(
            verify_proof(
                &again,
                &request("https://kuria.example/graphql", Some("the-token"), &jkt)
            )
            .unwrap()
            .jti,
            verified.jti
        );
    }

    #[test]
    fn a_saved_key_is_the_same_key() {
        let key = DpopKey::generate();
        let saved = DpopKey::import(&key.export()).unwrap();
        assert_eq!(saved.thumbprint(), key.thumbprint());
        assert!(DpopKey::import("not a key").is_err());
    }
}
