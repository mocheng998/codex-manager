pub mod commands;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
