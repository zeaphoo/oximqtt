//! Command-line argument parsing for the OXIMQTT broker.
//!
//! Defines the `Options` struct using `clap::Parser` to handle CLI flags
//! such as config file path and node ID.

use clap::Parser;

use crate::utils::NodeId;

/// Command-line options for the OXIMQTT broker.
///
/// Parsed from CLI arguments using `clap`. These values override settings
/// from the configuration file where applicable.
#[derive(Parser, Debug, Clone, Default)]
#[command(disable_version_flag = true)]
pub struct Options {
    /// Prints version information
    #[arg(short = 'V', long = "version")]
    pub version: bool,

    /// Config filename
    #[arg(short = 'f', long = "config")]
    pub cfg_name: Option<String>,

    /// Node id
    #[arg(long = "id")]
    pub node_id: Option<NodeId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let opts = Options::parse_from(["test"]);
        assert!(!opts.version);
        assert!(opts.cfg_name.is_none());
        assert!(opts.node_id.is_none());
    }

    #[test]
    fn test_version_short() {
        let opts = Options::parse_from(["test", "-V"]);
        assert!(opts.version);
    }

    #[test]
    fn test_version_long() {
        let opts = Options::parse_from(["test", "--version"]);
        assert!(opts.version);
    }

    #[test]
    fn test_config_short() {
        let opts = Options::parse_from(["test", "-f", "oximqtt.toml"]);
        assert_eq!(opts.cfg_name.as_deref(), Some("oximqtt.toml"));
    }

    #[test]
    fn test_config_long() {
        let opts = Options::parse_from(["test", "--config", "/etc/oximqtt/oximqtt.toml"]);
        assert_eq!(opts.cfg_name.as_deref(), Some("/etc/oximqtt/oximqtt.toml"));
    }

    #[test]
    fn test_node_id() {
        let opts = Options::parse_from(["test", "--id", "42"]);
        assert_eq!(opts.node_id, Some(42));
    }

    #[test]
    fn test_all_options() {
        let opts = Options::parse_from(["test", "-V", "-f", "config.toml", "--id", "99"]);
        assert!(opts.version);
        assert_eq!(opts.cfg_name.as_deref(), Some("config.toml"));
        assert_eq!(opts.node_id, Some(99));
    }
}
