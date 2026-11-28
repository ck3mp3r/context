//! Pins the scoped-liveness rule: the `should_refetch` truth table.
//!
//! The wasm crate cannot host-link its own test harness, so the pure module is included by path and
//! compiled into this binary: no wasm, no browser, and no rendering. The views call this one
//! predicate before they schedule a refetch, so the table below is the whole rule.

#[path = "../src/tabs/liveness.rs"]
mod liveness;

use liveness::should_refetch;

#[test]
fn an_unloaded_hidden_view_never_refetches() {
    assert!(
        !should_refetch(false, false, true),
        "a panel the user never opened fetches nothing, even on its own kind of message"
    );
    assert!(
        !should_refetch(false, false, false),
        "and nothing at all on another entity's message"
    );
}

#[test]
fn a_loaded_hidden_view_refetches_on_its_own_entity() {
    assert!(
        should_refetch(true, false, true),
        "a hidden tab keeps the data it already loaded live (decision 15)"
    );
    assert!(
        !should_refetch(true, false, false),
        "but another entity's message refetches nothing (criterion 1)"
    );
}

#[test]
fn an_active_view_refetches_before_its_first_response() {
    assert!(
        should_refetch(false, true, true),
        "the view on screen refetches, so a message never races its first fetch"
    );
    assert!(
        !should_refetch(false, true, false),
        "a visible view still ignores another entity's message"
    );
}

#[test]
fn a_loaded_active_view_refetches_on_its_own_entity() {
    assert!(should_refetch(true, true, true));
    assert!(!should_refetch(true, true, false));
}

#[test]
fn the_message_match_is_never_overridden() {
    for has_loaded in [false, true] {
        for is_active in [false, true] {
            assert!(
                !should_refetch(has_loaded, is_active, false),
                "no loaded or active state makes another entity's message a refetch trigger"
            );
        }
    }
}

#[test]
fn an_active_or_loaded_view_is_the_only_refetch_case() {
    for has_loaded in [false, true] {
        for is_active in [false, true] {
            assert_eq!(
                should_refetch(has_loaded, is_active, true),
                has_loaded || is_active,
                "with a matching message, being loaded or on screen is the whole condition"
            );
        }
    }
}
