//! `bundle.json` (`bundle.format`, `bundle.contents`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use uuid::Uuid;

/// The bundle format this server writes. Format 1 is the same without
/// versions and commits, so it reads both.
pub const FORMAT: u32 = 2;

/// The formats this server reads.
pub const READS: [u32; 2] = [1, 2];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Selected akt types, with or without their akter.
    Types,
    /// Everything Kuria holds for the tenant.
    Tenant,
}

impl Scope {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Types => "types",
            Self::Tenant => "tenant",
        }
    }
}

/// What a bundle holds.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bundle {
    pub format: u32,
    pub scope: Scope,
    /// The akt type keys a `types` bundle selected.
    #[serde(default)]
    pub types: Vec<String>,
    pub holds_akter: bool,
    pub exported_at: DateTime<Utc>,
    pub akt_types: Vec<BundleType>,
    #[serde(default)]
    pub akter: Vec<BundleAkt>,
    #[serde(default)]
    pub memberships: Vec<BundleMembership>,
    #[serde(default)]
    pub links: Vec<BundleLink>,
    /// The tenant's number counters, in a `tenant` bundle.
    #[serde(default)]
    pub counters: Vec<BundleCounter>,
    #[serde(default)]
    pub contacts: Vec<BundleContact>,
    #[serde(default)]
    pub people: Vec<BundlePerson>,
    /// Who signed in to the tenant, in a `tenant` bundle.
    #[serde(default)]
    pub signed_in: Vec<BundleSignedIn>,
    #[serde(default)]
    pub attachments: Vec<BundleAttachment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleType {
    pub key: String,
    /// Oldest first; only the latest in a bundle without akter.
    pub versions: Vec<BundleVersion>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleVersion {
    pub id: Uuid,
    pub version: i32,
    pub definition: Json,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleAkt {
    pub id: Uuid,
    pub number: String,
    /// The `id` of the akt type version it was created with.
    pub type_version: Uuid,
    pub state: String,
    pub fields: Json,
    pub title: Option<String>,
    pub rank: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Oldest first.
    pub revisions: Vec<BundleRevision>,
    /// The attachments its fields hold, by field.
    #[serde(default)]
    pub attachments: Vec<BundleHeld>,
    /// The `id` of the akt in the bundle it was copied from, if it is a
    /// copy of one there (`akt.copy.record`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<Uuid>,
    /// Its versions, oldest first (`version.*`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<BundleAktVersion>,
    /// The diagrams it holds (`akt.field.markdown.diagrams`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagrams: Vec<BundleDiagram>,
}

/// A diagram an akt holds: its field, the hash of the source rendered, in
/// hexadecimal, and the attachment of its SVG.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleDiagram {
    pub field: String,
    pub source_sha256: String,
    pub attachment: Uuid,
}

/// A version of an akt: a revision it named by releasing it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleAktVersion {
    pub number: i32,
    pub revision: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub released_by: Uuid,
    pub released_at: DateTime<Utc>,
    /// What each reference naming no version meant at release
    /// (`version.pins`), as stored.
    pub pins: Json,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleRevision {
    pub revision: i32,
    pub author: Uuid,
    pub transition: Option<String>,
    pub reason: Option<String>,
    pub changes: Json,
    pub state: String,
    /// In hexadecimal.
    pub hash: String,
    pub created_at: DateTime<Utc>,
    pub membership: Option<Json>,
    /// The git commit a pushed batch made it from (`git.identity`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<Json>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleHeld {
    pub field: String,
    pub attachment: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleMembership {
    pub container: Uuid,
    pub member: Uuid,
    pub added_at: DateTime<Utc>,
    pub added_by: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleLink {
    pub source: Uuid,
    pub kind: String,
    pub target: Uuid,
    pub pinned_revision: Option<i32>,
    pub added_at: DateTime<Utc>,
    pub added_by: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleCounter {
    pub name: String,
    pub value: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleContact {
    pub id: Uuid,
    pub name: Option<String>,
    pub email: Option<String>,
    pub linked_subject: Option<Uuid>,
    pub linked_at: Option<DateTime<Utc>>,
    pub linked_by: Option<Uuid>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundlePerson {
    pub subject: Uuid,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleSignedIn {
    pub subject: Uuid,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleAttachment {
    pub id: Uuid,
    pub name: String,
    pub media_type: String,
    pub size: i64,
    /// In hexadecimal; its bytes are `attachments/<sha256>`.
    pub sha256: String,
    pub uploaded_by: Uuid,
    pub uploaded_at: DateTime<Utc>,
    pub sbom: Option<Json>,
}
