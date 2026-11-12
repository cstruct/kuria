//! Asking Konklav what a request's subject may do with akt types and akter.
//!
//! Each permission becomes a set of Konklav requirements, decided by the
//! relying SDK for the request's bearer token. Decisions are remembered for
//! the rest of the request only.

use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use konklav_authz::{Decision, Requirements, ScopeRequirement};
use konklav_relying_client::RelyingClient;

/// Defining akt types, tenant-wide.
pub const DEFINE_TYPES: &str = "kuria:akt_types:define";
/// Seeing an akt type's akter.
pub const READ: &str = "kuria:akter:read";
/// Creating an akt type's akter.
pub const CREATE: &str = "kuria:akter:create";
/// Editing an akt type's akter.
pub const EDIT: &str = "kuria:akter:edit";
/// Performing an akt type's transitions.
pub const TRANSITION: &str = "kuria:akter:transition";
/// The scope kind of an akt type, whose ids are akt type keys.
pub const SCOPE_KIND: &str = "kuria_akt_type";

type DecisionFuture<'a> = Pin<Box<dyn Future<Output = Result<Decision, String>> + Send + 'a>>;

/// What decides requirements for a bearer token: the relying client, or a
/// stand-in in tests.
pub trait Decider: Send + Sync + 'static {
    fn decide<'a>(&'a self, token: &'a str, requirements: &'a Requirements) -> DecisionFuture<'a>;
}

impl Decider for RelyingClient {
    fn decide<'a>(&'a self, token: &'a str, requirements: &'a Requirements) -> DecisionFuture<'a> {
        Box::pin(async move {
            RelyingClient::decide(self, token, requirements)
                .await
                .map_err(|e| e.to_string())
        })
    }
}

/// A permission on akt types or akter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Permission {
    DefineTypes,
    Read { key: String },
    Create { key: String },
    Edit { key: String },
    Transition { key: String, action: Option<String> },
}

impl Permission {
    /// The Konklav requirements the permission stands for.
    // r[impl authz.catalog]
    fn requirements(&self) -> Requirements {
        let at_type = |key: &str, actions: &[&str]| Requirements {
            scopes: vec![ScopeRequirement {
                tenant: None,
                kind: SCOPE_KIND.to_owned(),
                id: key.to_owned(),
                actions: actions.iter().map(|a| (*a).to_owned()).collect(),
            }],
            ..Requirements::default()
        };
        match self {
            Self::DefineTypes => Requirements {
                tenant: BTreeSet::from([DEFINE_TYPES.to_owned()]),
                ..Requirements::default()
            },
            Self::Read { key } => at_type(key, &[READ]),
            Self::Create { key } => at_type(key, &[CREATE]),
            Self::Edit { key } => at_type(key, &[EDIT]),
            // r[impl authz.transition-action]
            Self::Transition { key, action } => {
                let mut actions = vec![TRANSITION];
                if let Some(action) = action {
                    actions.push(action);
                }
                at_type(key, &actions)
            }
        }
    }
}

/// The relying SDK could not decide.
#[derive(Debug, thiserror::Error)]
#[error("authorization is unavailable: {0}")]
pub struct Unavailable(pub String);

/// Decides permissions for one request's token.
pub struct Authorizer {
    decider: Arc<dyn Decider>,
    token: String,
    decided: Mutex<HashMap<Permission, bool>>,
}

impl Authorizer {
    #[must_use]
    pub fn new(decider: Arc<dyn Decider>, token: String) -> Self {
        Self {
            decider,
            token,
            decided: Mutex::default(),
        }
    }

    /// Whether the request's subject has `permission`. Fails closed.
    // r[impl authz.decide]
    pub async fn allows(&self, permission: &Permission) -> Result<bool, Unavailable> {
        if let Some(allowed) = self
            .decided
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(permission)
        {
            return Ok(*allowed);
        }
        let decision = self
            .decider
            .decide(&self.token, &permission.requirements())
            .await
            .map_err(Unavailable)?;
        // A step-up is a refusal here: nothing Kuria asks for requires one.
        let allowed = matches!(decision, Decision::Allow { .. });
        self.decided
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(permission.clone(), allowed);
        Ok(allowed)
    }

    /// Whether the subject may see the akt type `key`: may define akt types,
    /// or read its akter.
    // r[impl authz.read-types]
    pub async fn sees_type(&self, key: &str) -> Result<bool, Unavailable> {
        Ok(self.allows(&Permission::DefineTypes).await?
            || self
                .allows(&Permission::Read {
                    key: key.to_owned(),
                })
                .await?)
    }
}
