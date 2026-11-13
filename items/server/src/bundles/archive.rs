//! A bundle on the wire: a tar archive of its directory (`bundle.format`),
//! which an import's may also hold `subjects.json`
//! (`bundle.import.request`).

use std::collections::HashMap;
use std::io::Read as _;

use super::BundleError;

/// What an archive held.
#[derive(Debug, Default)]
pub struct Unpacked {
    pub bundle: Vec<u8>,
    /// Bytes by SHA-256 in hex.
    pub attachments: HashMap<String, Vec<u8>>,
    pub subjects: Option<Vec<u8>>,
}

/// A tar archive of `bundle.json` and `attachments/<sha256>`.
pub fn pack(bundle: &[u8], attachments: &[(String, Vec<u8>)]) -> Result<Vec<u8>, BundleError> {
    let internal = |e: std::io::Error| BundleError::Internal(format!("packing a bundle: {e}"));
    let mut builder = tar::Builder::new(Vec::new());
    let mut add = |path: &str, bytes: &[u8]| {
        let mut header = tar::Header::new_ustar();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        builder.append_data(&mut header, path, bytes)
    };
    add("bundle.json", bundle).map_err(internal)?;
    for (sha256, bytes) in attachments {
        add(&format!("attachments/{sha256}"), bytes).map_err(internal)?;
    }
    builder.into_inner().map_err(internal)
}

/// What `archive` holds: its `bundle.json`, attachments and any
/// `subjects.json`. Other entries are left alone.
pub fn unpack(archive: &[u8]) -> Result<Unpacked, BundleError> {
    let invalid = |e: std::io::Error| BundleError::Invalid(format!("not a tar archive: {e}"));
    let mut unpacked = Unpacked::default();
    let mut found = false;
    let mut entries = tar::Archive::new(archive);
    for entry in entries.entries().map_err(invalid)? {
        let mut entry = entry.map_err(invalid)?;
        if entry.header().entry_type() != tar::EntryType::Regular {
            continue;
        }
        let path = entry
            .path()
            .map_err(invalid)?
            .to_string_lossy()
            .into_owned();
        let path = path.trim_start_matches("./").to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(invalid)?;
        match path.as_str() {
            "bundle.json" => {
                unpacked.bundle = bytes;
                found = true;
            }
            "subjects.json" => unpacked.subjects = Some(bytes),
            _ => {
                if let Some(sha256) = path.strip_prefix("attachments/") {
                    unpacked
                        .attachments
                        .insert(sha256.to_ascii_lowercase(), bytes);
                }
            }
        }
    }
    if !found {
        return Err(BundleError::Invalid(
            "the archive holds no `bundle.json`".to_owned(),
        ));
    }
    Ok(unpacked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_what_it_packs() {
        let packed = pack(b"{}", &[("ab12".to_owned(), b"bytes".to_vec())]).unwrap();
        let unpacked = unpack(&packed).unwrap();
        assert_eq!(unpacked.bundle, b"{}");
        assert_eq!(unpacked.attachments["ab12"], b"bytes");
        assert!(unpacked.subjects.is_none());
        assert!(matches!(unpack(b"not tar"), Err(BundleError::Invalid(_))));
        let empty = tar::Builder::new(Vec::new()).into_inner().unwrap();
        assert!(matches!(unpack(&empty), Err(BundleError::Invalid(_))));
    }
}
