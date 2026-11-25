//! Pins the route classifier: which pathname is chrome, which names a tab, and which is inert.
//!
//! The wasm crate cannot host-link its own test harness, so the modules this one needs are included
//! by path and compiled into this binary: no wasm, no browser, and no router.

// The model modules are included whole, so the parts this binary does not pin are dead here.
#![allow(dead_code)]

#[path = "../src/tabs/open.rs"]
mod open;
#[path = "../src/tabs/pure.rs"]
mod pure;
#[path = "../src/tabs/registry.rs"]
mod registry;
#[path = "../src/tabs/route.rs"]
mod route;
#[path = "../src/tabs/session.rs"]
mod session;

use route::{RouteKind, route_kind};

#[test]
fn the_projects_home_is_the_only_projects_home() {
    assert_eq!(route_kind("/"), RouteKind::ProjectsHome);
    assert_eq!(
        route_kind(""),
        RouteKind::ProjectsHome,
        "an empty path is the projects home"
    );
    assert_eq!(
        route_kind("/projects"),
        RouteKind::Unknown,
        "the projects list with no entity is no route the app renders"
    );
}

#[test]
fn every_chrome_global_route_is_chrome_global() {
    for path in ["/notes", "/repos", "/skills", "/skills/abc", "/settings"] {
        assert_eq!(
            route_kind(path),
            RouteKind::ChromeGlobal,
            "{path} is a chrome list, so it hides every tab"
        );
    }
    assert_eq!(
        route_kind("/notes/"),
        RouteKind::ChromeGlobal,
        "a trailing slash names the same list"
    );
}

#[test]
fn every_path_a_tab_names_is_a_tab_surface() {
    for path in [
        "/projects/p1",
        "/projects/p1/task-lists/t1",
        "/projects/p1/notes/n1",
        "/projects/p1/repos/r1",
        "/task-lists/t1",
        "/notes/n1",
        "/repos/r1",
    ] {
        assert_eq!(
            route_kind(path),
            RouteKind::TabSurface,
            "{path} names a surface a tab renders"
        );
    }
}

#[test]
fn a_path_the_app_has_no_route_for_is_unknown() {
    assert_eq!(route_kind("/some/other/route"), RouteKind::Unknown);
    assert_eq!(
        route_kind("/projects/p1/task-lists"),
        RouteKind::Unknown,
        "a list path under a project is no surface"
    );
    assert_eq!(route_kind("/skills/abc/def"), RouteKind::Unknown);
}

#[test]
fn only_chrome_routes_deactivate_the_registry() {
    assert!(
        RouteKind::ProjectsHome.is_chrome(),
        "the projects overview deactivates too: no chip is active there"
    );
    assert!(RouteKind::ChromeGlobal.is_chrome());
    assert!(!RouteKind::TabSurface.is_chrome());
    assert!(
        !RouteKind::Unknown.is_chrome(),
        "a URL the app did not write must move no active tab, so pushState and Back stay inert"
    );
}
