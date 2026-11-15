//! The Kuria server: a GraphQL API over HTTP, authenticated through Konklav.
//!
//! [`app`] builds the router from a relying host, so tests can stand in for
//! Konklav. The binary in `main.rs` builds one from [`Config`], with a real
//! relying client.

pub mod akter;
pub mod attachments;
pub mod authorize;
pub mod bundles;
pub mod changes;
pub mod cli_relay;
mod config;
pub mod erasure;
mod graphql;
mod http;
pub mod live;
pub mod oidc;
pub mod pdf;
pub mod session;
pub mod web;

pub use config::{Config, ConfigError, StorageConfig};
pub use graphql::{KuriaSchema, schema, schema_sdl};
pub use http::{AppError, Changes, Options, app};
