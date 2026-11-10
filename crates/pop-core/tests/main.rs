//! The core's integration tests, as one binary: each module once linked the whole
//! dependency graph on its own. nextest still runs every test in its own process.
use pop_testkit as support;

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
mod publications;
mod replace;
mod shape;
mod sync;
mod text;
mod theme;
mod tokens;
mod trimmed;
mod undo;
mod validation;
