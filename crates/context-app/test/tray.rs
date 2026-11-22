//! Tests for the tray menu labels.

use super::*;

#[cfg(debug_assertions)]
#[test]
fn debug_profile_mcp_label_names_the_dev_endpoint() {
    assert_eq!(mcp_label(), "MCP: http://localhost:3738/mcp");
}

#[cfg(not(debug_assertions))]
#[test]
fn release_profile_mcp_label_names_the_production_endpoint() {
    assert_eq!(mcp_label(), "MCP: http://localhost:3737/mcp");
}

#[test]
fn mcp_label_matches_the_port_the_app_serves() {
    assert!(
        mcp_label().ends_with(&format!(":{}/mcp", crate::api_port())),
        "label was: {}",
        mcp_label()
    );
}
