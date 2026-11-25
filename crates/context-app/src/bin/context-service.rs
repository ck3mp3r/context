//! `context-service`: windowless service for c5t.
//!
//! Owns the tray and the API server. It never creates a window and never builds
//! an application menu: the GUI menus exist for the webview responder chain, and
//! this binary has no webview. It bakes `service/tauri.conf.json`, so its
//! identifier — and therefore its single-instance socket — is its own, and the
//! GUI never kills it or the other way round.
//!
//! The tray drives the two halves: Show and Settings bring the GUI forward, and
//! Quit ends the GUIs this service spawned before exiting.

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use context_app::{ServiceState, ServiceUpdateFlow, autostart, effective_settings, server, tray};

fn main() -> tauri::Result<()> {
    let app = tauri::Builder::default()
        // A second service exits through the plugin; there is no window to raise.
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .on_menu_event(|app, event| tray::handle_menu_event(app, event.id().as_ref()))
        .setup(|app| {
            // A legacy XDG config file reaches the native root before the port
            // decision reads it.
            context_core::settings::migrate_legacy_config();

            #[cfg(target_os = "macos")]
            app.handle()
                .set_activation_policy(tauri::ActivationPolicy::Accessory)?;

            let port = server::api_port();
            let health_url = server::health_url(port);

            let notifier = context_server::api::notifier::ChangeNotifier::new();
            let flow = ServiceUpdateFlow::new(
                app.handle().clone(),
                notifier.clone(),
                effective_settings,
                context_core::update::update_state_path(),
            );
            flow.reconcile_startup();

            if let Err(message) = tauri::async_runtime::block_on(server::start_server(
                port,
                &health_url,
                notifier,
                flow,
            )) {
                eprintln!("context-service: {message}");
                app.dialog()
                    .message(message)
                    .title("context-service")
                    .kind(MessageDialogKind::Error)
                    .blocking_show();
                std::process::exit(1);
            }

            app.manage(ServiceState::new());
            app.manage(tray::build(app.handle())?);
            tray::spawn_status_poll(app.handle().clone());

            // A login item must never block the server: log and serve on.
            if let Err(error) = autostart::sync_start_at_login(effective_settings().start_at_login)
            {
                eprintln!("context-service: {error}");
            }

            Ok(())
        })
        .build(tauri::generate_context!("service/tauri.conf.json"))?;

    app.run(|_app, _event| {});

    Ok(())
}
