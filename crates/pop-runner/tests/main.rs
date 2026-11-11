//! The runner's integration tests, as one binary. `broken_pipe` stays its own: it changes
//! the process's SIGPIPE disposition.
#[cfg(feature = "in-process")]
mod authority;
mod process;
