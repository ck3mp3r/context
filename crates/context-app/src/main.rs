//! Tauri desktop shell for c5t.
//!
//! Wires the shared `context_app` modules into a Tauri app: the server is
//! attached or started in-process for the current build profile, then the window
//! loads the SPA that talks to it.

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use context_app::{lifecycle, menu, server, tray};

fn main() -> tauri::Result<()> {
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
            } else if event.id() == tray::SHOW_ITEM_ID {
                lifecycle::request_main_window(app);
            } else if event.id() == tray::SETTINGS_ITEM_ID {
                lifecycle::request_window_at(app, lifecycle::SETTINGS_PATH);
            }
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.handle()
                .set_activation_policy(tauri::ActivationPolicy::Accessory)?;

            let port = server::api_port();
            let health_url = server::health_url(port);

            if let Err(message) =
                tauri::async_runtime::block_on(server::start_server(port, &health_url))
            {
                eprintln!("context: {message}");
                app.dialog()
                    .message(message)
                    .title("context")
                    .kind(MessageDialogKind::Error)
                    .blocking_show();
                std::process::exit(1);
            }

            lifecycle::create_main_window(app.handle())?;
            menu::build(app.handle())?;
            app.manage(tray::build(app.handle())?);
            tray::spawn_status_poll(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())?;

    app.run(lifecycle::handle_run_event);

    Ok(())
}

/// Close the main window and report a failure without stopping the app.
///
/// The application-menu item and its Cmd+Q accelerator land here. A failure to
/// close the view must not take the service down, so it is logged.
fn close_view(app: &tauri::AppHandle) {
    if let Err(error) = lifecycle::close_main_window(app) {
        eprintln!("context: failed to close the main window: {error}");
    }
}
