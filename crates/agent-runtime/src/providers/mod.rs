//! Parallel implementations of the same provider contract.
pub(crate) mod carbot;
pub(crate) mod claude;
pub(crate) mod codex;
mod launch_command;
pub(crate) mod mock;
mod process;

mod provider;
pub(crate) use provider::Provider;
