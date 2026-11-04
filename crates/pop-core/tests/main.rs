//! The core's integration tests, as one binary: each module once linked the whole
//! dependency graph on its own. nextest still runs every test in its own process.
mod support;

mod app_definition;
mod arguments;
mod collections;
mod conformance;
mod counters;
mod defaults;
mod merge;
mod metadata;
mod model;
mod nested;
mod page_wire;
mod publications;
mod replace;
mod shape;
mod socket_wire;
mod sync;
mod text;
mod theme;
mod tokens;
mod trimmed;
mod undo;
mod validation;
