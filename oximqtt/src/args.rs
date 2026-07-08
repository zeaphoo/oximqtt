//! Command-line argument types for the OXIMQTT broker.
//!
//! This module defines the data structures used to parse and represent
//! command-line arguments, such as node identity.

use crate::types::NodeId;

/// Parsed command-line arguments for the OXIMQTT broker.
///
/// Holds optional overrides for node identity.
/// These values can be supplied via CLI flags to override the main
/// configuration file.
#[derive(Debug, Clone, Default)]
pub struct CommandArgs {
    /// Node id
    pub node_id: Option<NodeId>,
}
