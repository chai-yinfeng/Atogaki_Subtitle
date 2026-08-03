//! Reusable local processing core for Atogaki.
//!
//! The CLI is one interface over this library. A future desktop UI should use
//! the same application services instead of invoking media tooling directly.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
