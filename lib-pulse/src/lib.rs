//! Pulse: one interface over every platform's package manager, plus a
//! first-class path for installing binaries directly.
//!
//! The front end talks to [`Registry`] and the high-level helpers in
//! [`pulse`]; each supported package manager is a [`Backend`] living in its
//! own module.

pub mod archive;
pub mod backend;
pub mod config;
pub mod db;
pub mod networking;
pub mod paths;
pub mod platform;
pub mod process;
pub mod update;

pub mod apt;
pub mod direct;
pub mod dnf;
pub mod homebrew;
pub mod pacman;
pub mod winget;

/// High-level operations that route work across the available backends.
pub mod pulse;

pub use backend::{Backend, Package, Registry};
