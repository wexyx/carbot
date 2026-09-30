//! Parallel implementations of the same provider contract.
pub(crate) mod carbot;
pub(crate) mod claude;
pub(crate) mod codex;
mod home;
mod launch_command;
pub(crate) mod mock;
pub(crate) mod opencode;
mod process;

mod provider;
pub(crate) use provider::Provider;
