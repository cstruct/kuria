//! Change events from the outbox to subscribers.
//!
//! - [`relay`]: publishes committed change events to Valkey.
//! - [`hub`]: wakes on Valkey, reads change events from Postgres in order,
//!   and broadcasts them to subscriptions, one hub per tenant.

mod hub;
mod relay;

pub use hub::{Hub, Subscription};
pub use relay::{Relay, RelayError, stream_key};
