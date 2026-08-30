//! Pulse: one native interface over every platform's software sources, plus a
//! first-class path for installing binaries directly. Nothing shells out — all
//! package logic is built in.
//!
//! The front end talks to [`Registry`] and the high-level helpers in [`ops`];
//! each source is a [`Backend`] living in its own module.

pub mod archive;
pub mod backend;
pub mod config;
pub mod db;
pub mod mode;
pub mod native;
pub mod networking;
pub mod paths;
pub mod platform;
pub mod process;
pub mod progress;
pub mod update;
pub mod wine;

pub mod apt;
pub mod direct;
pub mod dnf;
pub mod homebrew;
pub mod macports;
pub mod msys2;
pub mod pacman;
pub mod registry;
pub mod winget;

/// High-level operations that route work across the available sources.
pub mod ops;

pub use backend::{Backend, Describe, Package, Registry};
