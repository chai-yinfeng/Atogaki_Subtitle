//! Reusable local processing core for Atogaki.
//!
//! The CLI and Tauri desktop share these application services. The legacy
//! Web interface remains a compatibility entry point, separate from the desktop.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
