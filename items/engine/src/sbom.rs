//! Recognising software bills of materials.

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// What Kuria keeps about an SBOM.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomSummary {
    /// `spdx` or `cyclonedx`.
    pub format: SbomFormat,
    /// The specification version, such as `SPDX-2.3` or `1.6`.
    pub spec_version: String,
    /// The document's name, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// How many packages (SPDX) or components (CycloneDX) it lists.
    pub components: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SbomFormat {
    Spdx,
    Cyclonedx,
}

/// The summary of `bytes` if they are an SPDX or CycloneDX JSON document,
/// else `None`.
// r[impl attachment.sbom]
#[must_use]
pub fn detect(bytes: &[u8]) -> Option<SbomSummary> {
    let document: Json = serde_json::from_slice(bytes).ok()?;
    let object = document.as_object()?;
    let text = |value: Option<&Json>| value.and_then(Json::as_str).map(str::to_owned);
    let count = |value: Option<&Json>| value.and_then(Json::as_array).map_or(0, Vec::len);
    if let Some(version) = text(object.get("spdxVersion")) {
        return Some(SbomSummary {
            format: SbomFormat::Spdx,
            spec_version: version,
            name: text(object.get("name")),
            components: count(object.get("packages")),
        });
    }
    if object.get("bomFormat").and_then(Json::as_str) == Some("CycloneDX") {
        return Some(SbomSummary {
            format: SbomFormat::Cyclonedx,
            spec_version: text(object.get("specVersion"))?,
            name: text(
                object
                    .get("metadata")
                    .and_then(|m| m.get("component"))
                    .and_then(|c| c.get("name")),
            ),
            components: count(object.get("components")),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // r[verify attachment.sbom]
    #[test]
    fn recognises_spdx_and_cyclonedx() {
        let spdx = json!({
            "spdxVersion": "SPDX-2.3", "name": "backend-4.2.0",
            "packages": [{ "name": "serde" }, { "name": "tokio" }]
        });
        assert_eq!(
            detect(spdx.to_string().as_bytes()),
            Some(SbomSummary {
                format: SbomFormat::Spdx,
                spec_version: "SPDX-2.3".to_owned(),
                name: Some("backend-4.2.0".to_owned()),
                components: 2,
            })
        );
        let cyclonedx = json!({
            "bomFormat": "CycloneDX", "specVersion": "1.6",
            "metadata": { "component": { "name": "backend" } },
            "components": [{ "name": "serde" }]
        });
        assert_eq!(
            detect(cyclonedx.to_string().as_bytes()),
            Some(SbomSummary {
                format: SbomFormat::Cyclonedx,
                spec_version: "1.6".to_owned(),
                name: Some("backend".to_owned()),
                components: 1,
            })
        );
    }

    // r[verify attachment.sbom]
    #[test]
    fn nothing_else_is_an_sbom() {
        for bytes in [
            b"not json".as_slice(),
            br#"{"bomFormat": "Other", "specVersion": "1"}"#,
            br#"{"bomFormat": "CycloneDX"}"#,
            br#"["spdxVersion"]"#,
            b"\x89PNG\r\n",
        ] {
            assert_eq!(detect(bytes), None, "{}", String::from_utf8_lossy(bytes));
        }
    }
}
