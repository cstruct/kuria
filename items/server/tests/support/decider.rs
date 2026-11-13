//! A stand-in for the relying client's decisions: actions granted per token,
//! tenant-wide or at one akt type.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use konklav_authz::{Decision, DenyReason, Requirements};
use kuria_server::authorize::{Decider, SCOPE_KIND};

#[derive(Default)]
struct Grants {
    everything: bool,
    tenant: BTreeSet<String>,
    at_type: HashMap<String, BTreeSet<String>>,
}

/// Decides as Konklav would for grants given per token: an action required
/// at an akt type is satisfied tenant-wide or at that type.
#[derive(Default)]
pub struct FakeDecider {
    grants: Mutex<HashMap<String, Grants>>,
    failing: Mutex<bool>,
    decisions: Mutex<u32>,
}

impl FakeDecider {
    /// A decider that allows the token `tenant` everything.
    pub fn allowing_tenant() -> Arc<Self> {
        let decider = Arc::new(Self::default());
        decider
            .grants
            .lock()
            .unwrap()
            .entry("tenant".to_owned())
            .or_default()
            .everything = true;
        decider
    }

    /// Allows `token` everything, as `allowing_tenant` does `tenant`.
    pub fn allow_everything(&self, token: &str) {
        self.grants
            .lock()
            .unwrap()
            .entry(token.to_owned())
            .or_default()
            .everything = true;
    }

    /// Grants `token` the `actions` tenant-wide.
    pub fn grant(&self, token: &str, actions: &[&str]) {
        let mut grants = self.grants.lock().unwrap();
        let grants = grants.entry(token.to_owned()).or_default();
        grants
            .tenant
            .extend(actions.iter().map(|a| (*a).to_owned()));
    }

    /// Grants `token` the `actions` at the akt type `key`.
    pub fn grant_at(&self, token: &str, key: &str, actions: &[&str]) {
        let mut grants = self.grants.lock().unwrap();
        let grants = grants.entry(token.to_owned()).or_default();
        grants
            .at_type
            .entry(key.to_owned())
            .or_default()
            .extend(actions.iter().map(|a| (*a).to_owned()));
    }

    pub fn set_failing(&self, failing: bool) {
        *self.failing.lock().unwrap() = failing;
    }

    /// How many decisions it has made.
    pub fn decisions(&self) -> u32 {
        *self.decisions.lock().unwrap()
    }

    pub fn decide_now(&self, token: &str, requirements: &Requirements) -> Result<Decision, String> {
        *self.decisions.lock().unwrap() += 1;
        if *self.failing.lock().unwrap() {
            return Err("Konklav is unreachable".to_owned());
        }
        let grants = self.grants.lock().unwrap();
        let Some(grants) = grants.get(token) else {
            return Ok(deny(requirements.tenant.clone()));
        };
        if grants.everything {
            return Ok(Decision::allow());
        }
        let mut missing: BTreeSet<String> = requirements
            .tenant
            .difference(&grants.tenant)
            .cloned()
            .collect();
        for scope in &requirements.scopes {
            assert_eq!(scope.kind, SCOPE_KIND, "Kuria asks only at akt types");
            let at = grants.at_type.get(&scope.id);
            missing.extend(
                scope
                    .actions
                    .iter()
                    .filter(|a| {
                        !grants.tenant.contains(*a) && !at.is_some_and(|at| at.contains(*a))
                    })
                    .cloned(),
            );
        }
        Ok(if missing.is_empty() {
            Decision::allow()
        } else {
            deny(missing)
        })
    }
}

fn deny(actions: BTreeSet<String>) -> Decision {
    Decision::Deny {
        reason: DenyReason::MissingActions { actions },
    }
}

impl Decider for FakeDecider {
    fn decide<'a>(
        &'a self,
        token: &'a str,
        requirements: &'a Requirements,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Decision, String>> + Send + 'a>>
    {
        Box::pin(async move { self.decide_now(token, requirements) })
    }
}
