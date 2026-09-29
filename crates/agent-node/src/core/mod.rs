mod approvals;
mod capabilities;
pub(crate) mod conversation;
pub(crate) mod error;
pub(crate) mod events;
mod group_configuration;
pub(crate) mod local;
pub(crate) mod messages;
mod network;
pub(crate) mod policies;
pub(crate) mod relay;
mod service;
pub(crate) mod skills;
pub(crate) mod state;
pub(crate) mod tool_policy;
pub(crate) use service::Core;
pub(crate) mod group_commands;

mod project_execution;
mod project_workspace;

mod agent_configuration;
mod agent_directory;

mod agent_deletion;
mod agent_tests;
mod default_agent;
mod project_titles;
mod remote_agents;
mod virtual_agents;

mod connections;

mod chat_management;

mod remote_directory;

pub(crate) mod context_reset;

mod permissions;

mod member_events;

mod command_allowlist;

mod response_instructions;
