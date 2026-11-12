//! The Kuria server: a GraphQL API over HTTP, authenticated through Konklav.
//!
//! [`app`] builds the router from a relying host, so tests can stand in for
//! Konklav. The binary in `main.rs` builds one from [`Config`], with a real
//! relying client.

mod config;
pub mod erasure;
mod graphql;
mod http;
pub mod oidc;
pub mod session;

pub use config::{Config, ConfigError};
pub use graphql::{KuriaSchema, schema};
pub use http::{AppError, Options, app};
