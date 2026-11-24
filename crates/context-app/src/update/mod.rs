//! Update machinery shared by the service and the GUI.
//!
//! [`releases`] is the pure GitHub release lookup: the updater plugin has no
//! channel logic, so the winning release is chosen here. The flow that drives
//! the plugin arrives with its own submodule.

pub mod releases;

#[cfg(test)]
#[path = "../../test/update/releases.rs"]
mod releases_test;
