//! The core's integration tests, as one binary: each module once linked the whole
//! dependency graph on its own. nextest still runs every test in its own process.
mod support;

mod app_definition;
mod arguments;
mod build_input;
mod collections;
#[cfg(feature = "storage")]
mod command;
#[cfg(feature = "storage")]
mod compat;
#[cfg(feature = "storage")]
mod compat_writers;
mod conformance;
mod counters;
mod defaults;
#[cfg(feature = "storage")]
mod engine_wire;
#[cfg(feature = "storage")]
mod file;
mod images;
mod media;
mod merge;
mod metadata;
mod model;
#[cfg(feature = "storage")]
mod native_wire;
mod nested;
mod owner;
mod owner_commands;
mod page_wire;
mod publications;
mod replace;
mod shape;
mod socket_wire;
#[cfg(feature = "storage")]
mod store;
mod sync;
mod text;
mod theme;
mod tokens;
mod trimmed;
mod undo;
mod validation;
