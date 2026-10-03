//! Real-console regressions: PTY input/output and a gated local provider, no account.
#![cfg(all(unix, feature = "tui"))]

#[path = "pty_console/harness.rs"]
mod harness;

#[cfg(test)]
#[path = "pty_console/tests.rs"]
mod tests;
