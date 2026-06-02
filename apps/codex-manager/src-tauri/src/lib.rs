pub mod commands;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let show_item =
                tauri::menu::MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit_item =
                tauri::menu::MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&show_item, &quit_item])?;
            let Some(icon) = app.default_window_icon().cloned() else {
                return Ok(());
            };

            tauri::tray::TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("Codex Manager")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::backend_version,
            commands::check_update,
            commands::load_settings,
            commands::save_settings,
            commands::login_user,
            commands::logout_user,
            commands::activate_login_account,
            commands::delete_login_account,
            commands::search_remote_keys,
            commands::decrypt_remote_key,
            commands::upsert_account,
            commands::delete_account,
            commands::activate_account,
            commands::apply_active_account,
            commands::clear_api_mode,
            commands::restore_backup,
            commands::read_restore_preview,
            commands::read_codex_config,
            commands::open_codex_install_page,
            commands::detect_codex_path,
            commands::launch_codex,
            commands::restart_codex
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Codex Manager");
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
