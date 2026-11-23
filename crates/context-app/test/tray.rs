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

#[cfg(debug_assertions)]
#[test]
fn debug_profile_tray_tooltip_names_the_dev_build() {
    assert_eq!(tray_tooltip(), "context (dev)");
}

#[cfg(not(debug_assertions))]
#[test]
fn release_profile_tray_tooltip_keeps_the_production_name() {
    assert_eq!(tray_tooltip(), "context");
}

#[test]
fn settings_item_carries_its_id_and_label() {
    assert_eq!(SETTINGS_ITEM_ID, "settings");
    assert_eq!(SETTINGS_TEXT, "Settings…");
}

fn parse_status(json: &str) -> SyncStatusResponse {
    serde_json::from_str(json).expect("Status response should deserialize")
}

#[test]
fn status_url_targets_the_port_the_app_serves() {
    assert_eq!(
        status_url(),
        format!("http://127.0.0.1:{}/api/v1/sync/status", crate::api_port())
    );
}

#[test]
fn sync_row_is_absent_before_sync_is_initialized() {
    let status = parse_status(
        r#"{"status":"not_initialized","message":"Sync not initialized. Run init operation first.","data":{"initialized":false,"last_synced_at":null,"schema_version":1}}"#,
    );

    assert_eq!(sync_label(status.data.as_ref()), None);
}

#[test]
fn sync_row_is_absent_when_the_response_carries_no_data() {
    let status = parse_status(r#"{"status":"success","message":"Sync status retrieved"}"#);

    assert_eq!(sync_label(status.data.as_ref()), None);
}

#[test]
fn sync_row_reports_a_clean_tree_and_the_last_sync_time() {
    let status = parse_status(
        r#"{"status":"success","message":"Sync status retrieved","data":{"initialized":true,"remote_url":"git@example.com:c5t-sync.git","last_synced_at":"2026-11-22 18:00:00","git":{"clean":true,"status":"No changes"}}}"#,
    );

    assert_eq!(
        sync_label(status.data.as_ref()).as_deref(),
        Some("Sync: clean · 2026-11-22 18:00:00")
    );
}

#[test]
fn sync_row_reports_never_synced_without_a_timestamp() {
    let status = parse_status(
        r#"{"status":"success","message":"Sync status retrieved","data":{"initialized":true,"remote_url":null,"last_synced_at":null,"git":{"clean":true,"status":"No changes"}}}"#,
    );

    assert_eq!(
        sync_label(status.data.as_ref()).as_deref(),
        Some("Sync: never synced")
    );
}

#[test]
fn sync_row_reports_pending_changes_for_a_dirty_tree() {
    let status = parse_status(
        r#"{"status":"success","message":"Sync status retrieved","data":{"initialized":true,"last_synced_at":"2026-11-22 18:00:00","git":{"clean":false,"status":" M tasks.jsonl"}}}"#,
    );

    assert_eq!(
        sync_label(status.data.as_ref()).as_deref(),
        Some("Sync: pending changes")
    );
}
