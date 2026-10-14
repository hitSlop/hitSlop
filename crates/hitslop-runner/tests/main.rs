//! The runner's integration tests, as one binary. `broken_pipe` stays its own: it changes
//! the process's SIGPIPE disposition.
mod process;
mod runner;
mod widgets;
