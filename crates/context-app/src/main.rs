//! Tauri desktop shell for c5t.
//!
//! The GUI is a pure view: it ensures a `context-service` answers on the
//! configured port, then loads the SPA that talks to it. It starts no server of
//! its own and owns no tray — quitting the GUI leaves the service running.
//!
//! `--open-route <path>` is the service's deep link: the window loads the SPA on
//! that path instead of the root.

use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use context_app::{launcher, lifecycle, menu, server};

fn main() -> tauri::Result<()> {
    // Read the route before the window exists: creation loads it.
    let route = launcher::open_route(std::env::args().skip(1));

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            lifecycle::request_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .on_menu_event(|app, event| {
            if event.id() == menu::QUIT_ITEM_ID {
                app.exit(0);
            } else if event.id() == menu::CLOSE_VIEW_ITEM_ID {
                close_view(app);
            }
        })
        .setup(move |app| {
            // Regular policy: the GUI owns a Dock icon and a Cmd+Tab entry.
            app.handle()
                .set_activation_policy(tauri::ActivationPolicy::Regular)?;

            let port = server::api_port();

            if let Err(message) = server::ensure_service(port) {
                eprintln!("context: {message}");
                app.dialog()
                    .message(message)
                    .title("Context")
                    .kind(MessageDialogKind::Error)
                    .blocking_show();
                std::process::exit(1);
            }

            lifecycle::create_main_window(app.handle(), route.as_deref())?;
            menu::build(app.handle())?;

            Ok(())
        })
        .build(tauri::generate_context!())?;

    app.run(lifecycle::handle_run_event);

    Ok(())
}

/// Close the main window and report a failure without stopping the app.
///
/// The window is a view over a service that outlives it, so a failure to close
/// it is logged rather than fatal.
fn close_view(app: &tauri::AppHandle) {
    if let Err(error) = lifecycle::close_main_window(app) {
        eprintln!("context: failed to close the main window: {error}");
    }
}
