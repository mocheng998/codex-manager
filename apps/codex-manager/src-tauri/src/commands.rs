use codex_manager_core::{
    Account, AppSettings, AuthState, BackupConfigPreview, CodexApplyResult, CodexConfigView,
    CodexPathInfo, LaunchRequest, LoginAccount, LoginCredentials, LoginPayload,
    RemoteKeyDecryptPayload, RemoteKeySearchPayload, SettingsStore, apply_account_to_codex,
    check_for_update_with_url, clear_codex_manager_config, launch_codex as launch_codex_core,
    login_new_api, read_codex_view, read_latest_backup_preview, resolve_codex_path,
    restart_codex as restart_codex_core, restore_latest_backup,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult<T>
where
    T: Serialize,
{
    pub status: String,
    pub message: String,
    #[serde(flatten)]
    pub payload: T,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionPayload {
    pub version: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub settings: AppSettings,
    pub settings_path: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAccountRequest {
    pub account: Account,
    #[serde(default)]
    pub activate: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRemoteKeysRequest {
    #[serde(default)]
    pub keyword: String,
    #[serde(default)]
    pub login_id: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptRemoteKeyRequest {
    #[serde(default, alias = "id")]
    pub token_id: String,
    #[serde(default)]
    pub login_id: String,
}

#[tauri::command]
pub fn backend_version() -> CommandResult<VersionPayload> {
    ok(
        "Backend version loaded",
        VersionPayload {
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
    )
}

#[tauri::command]
pub fn check_update() -> CommandResult<codex_manager_core::UpdateCheck> {
    let settings = SettingsStore::default().load().unwrap_or_default();
    match check_for_update_with_url(env!("CARGO_PKG_VERSION"), &settings.update_manifest_url) {
        Ok(payload) => ok(
            if payload.update_available {
                "发现可用更新"
            } else {
                "当前已是最新版本"
            },
            payload,
        ),
        Err(error) => failed(
            &format!("检查更新失败: {error}"),
            codex_manager_core::UpdateCheck {
                current_version: env!("CARGO_PKG_VERSION").to_string(),
                ..codex_manager_core::UpdateCheck::default()
            },
        ),
    }
}

#[tauri::command]
pub fn load_settings() -> CommandResult<SettingsPayload> {
    settings_payload("Settings loaded")
}

#[tauri::command]
pub fn save_settings(settings: AppSettings) -> CommandResult<SettingsPayload> {
    let store = SettingsStore::default();
    match store.save(&settings) {
        Ok(settings) => ok(
            "Settings saved",
            SettingsPayload {
                settings,
                settings_path: store.path().to_string_lossy().to_string(),
            },
        ),
        Err(error) => failed_payload("Settings save failed", error),
    }
}

#[tauri::command]
pub fn login_user(credentials: LoginCredentials) -> CommandResult<LoginPayload> {
    let store = SettingsStore::default();
    match login_new_api(credentials) {
        Ok(payload) => {
            let mut settings = store.load().unwrap_or_default();
            let mut login_account = settings
                .login_accounts
                .iter()
                .find(|account| account.matches_auth(&payload.auth))
                .cloned()
                .unwrap_or_else(|| LoginAccount::from_auth(payload.auth.clone()));
            login_account.auth = payload.auth.clone();

            if settings
                .login_accounts
                .iter()
                .any(|entry| entry.id == login_account.id)
            {
                settings.login_accounts = settings
                    .login_accounts
                    .into_iter()
                    .map(|entry| {
                        if entry.id == login_account.id {
                            login_account.clone()
                        } else {
                            entry
                        }
                    })
                    .collect();
            } else {
                settings.login_accounts.push(login_account.clone());
            }

            settings.active_login_id = login_account.id;
            settings.auth = payload.auth.clone();
            if let Err(error) = store.save(&settings) {
                return failed_payload("Login state save failed", error);
            }
            ok("登录成功", payload)
        }
        Err(error) => failed_payload("登录失败", error),
    }
}

#[tauri::command]
pub fn logout_user() -> CommandResult<SettingsPayload> {
    let store = SettingsStore::default();
    let mut settings = store.load().unwrap_or_default();
    if settings.active_login_id.is_empty() {
        settings.login_accounts.clear();
    } else {
        let active_login_id = settings.active_login_id.clone();
        settings
            .login_accounts
            .retain(|account| account.id != active_login_id);
    }
    settings.active_login_id = settings
        .login_accounts
        .first()
        .map(|account| account.id.clone())
        .unwrap_or_default();
    settings.auth = settings
        .active_login_account()
        .map(|account| account.auth.clone())
        .unwrap_or_default();
    match store.save(&settings) {
        Ok(settings) => ok(
            "已退出登录",
            SettingsPayload {
                settings,
                settings_path: store.path().to_string_lossy().to_string(),
            },
        ),
        Err(error) => failed_payload("Logout failed", error),
    }
}

#[tauri::command]
pub fn activate_login_account(id: String) -> CommandResult<SettingsPayload> {
    let store = SettingsStore::default();
    let mut settings = store.load().unwrap_or_default();
    if settings.login_account(&id).is_none() {
        return failed_payload("Login account not found", id);
    }
    settings.active_login_id = id;
    settings.auth = settings
        .active_login_account()
        .map(|account| account.auth.clone())
        .unwrap_or_default();
    match store.save(&settings) {
        Ok(settings) => ok(
            "Login account activated",
            SettingsPayload {
                settings,
                settings_path: store.path().to_string_lossy().to_string(),
            },
        ),
        Err(error) => failed_payload("Login account activate failed", error),
    }
}

#[tauri::command]
pub fn delete_login_account(id: String) -> CommandResult<SettingsPayload> {
    let store = SettingsStore::default();
    let mut settings = store.load().unwrap_or_default();
    settings.login_accounts.retain(|account| account.id != id);
    if settings.active_login_id == id {
        settings.active_login_id = settings
            .login_accounts
            .first()
            .map(|account| account.id.clone())
            .unwrap_or_default();
    }
    settings.auth = settings
        .active_login_account()
        .map(|account| account.auth.clone())
        .unwrap_or_default();
    match store.save(&settings) {
        Ok(settings) => ok(
            "Login account deleted",
            SettingsPayload {
                settings,
                settings_path: store.path().to_string_lossy().to_string(),
            },
        ),
        Err(error) => failed_payload("Login account delete failed", error),
    }
}

#[tauri::command]
pub fn search_remote_keys(
    request: SearchRemoteKeysRequest,
) -> CommandResult<RemoteKeySearchPayload> {
    let settings = match SettingsStore::default().load() {
        Ok(settings) => settings,
        Err(error) => return failed_payload("Settings load failed", error),
    };
    let auth = select_login_auth(&settings, &request.login_id);
    match codex_manager_core::search_remote_keys(&auth, &request.keyword) {
        Ok(payload) => ok("远程 KEY 已加载", payload),
        Err(error) => failed_payload("远程 KEY 查询失败", error),
    }
}

#[tauri::command]
pub fn decrypt_remote_key(
    request: DecryptRemoteKeyRequest,
) -> CommandResult<RemoteKeyDecryptPayload> {
    let settings = match SettingsStore::default().load() {
        Ok(settings) => settings,
        Err(error) => return failed_payload("Settings load failed", error),
    };
    let auth = select_login_auth(&settings, &request.login_id);
    match codex_manager_core::decrypt_remote_key(&auth, &request.token_id) {
        Ok(payload) => ok("KEY 解密成功", payload),
        Err(error) => failed_payload("KEY 解密失败", error),
    }
}

#[tauri::command]
pub fn upsert_account(request: UpsertAccountRequest) -> CommandResult<SettingsPayload> {
    let store = SettingsStore::default();
    let mut settings = store.load().unwrap_or_default();
    let mut account = request.account;
    if account.id.trim().is_empty() {
        account = Account::new(account.name, account.base_url, account.api_key);
    }
    if settings.accounts.iter().any(|entry| entry.id == account.id) {
        settings.accounts = settings
            .accounts
            .into_iter()
            .map(|entry| {
                if entry.id == account.id {
                    account.clone()
                } else {
                    entry
                }
            })
            .collect();
    } else {
        settings.accounts.push(account.clone());
    }
    if request.activate {
        settings.active_account_id = account.id;
    }
    save_settings(settings)
}

#[tauri::command]
pub fn delete_account(id: String) -> CommandResult<SettingsPayload> {
    let store = SettingsStore::default();
    let mut settings = store.load().unwrap_or_default();
    settings.accounts.retain(|account| account.id != id);
    if settings.active_account_id == id {
        settings.active_account_id.clear();
    }
    match store.save(&settings) {
        Ok(settings) => ok(
            "Account deleted",
            SettingsPayload {
                settings,
                settings_path: store.path().to_string_lossy().to_string(),
            },
        ),
        Err(error) => failed_payload("Account delete failed", error),
    }
}

#[tauri::command]
pub fn activate_account(id: String) -> CommandResult<CodexApplyResult> {
    let store = SettingsStore::default();
    let mut settings = match store.load() {
        Ok(settings) => settings,
        Err(error) => return failed_payload("Settings load failed", error),
    };
    settings.active_account_id = id;
    let settings = match store.save(&settings) {
        Ok(settings) => settings,
        Err(error) => return failed_payload("Settings save failed", error),
    };
    match settings.active_account() {
        Some(account) => apply_result(apply_account_to_codex(account)),
        None => failed_message("No active account is available"),
    }
}

#[tauri::command]
pub fn apply_active_account() -> CommandResult<CodexApplyResult> {
    let settings = match SettingsStore::default().load() {
        Ok(settings) => settings,
        Err(error) => return failed_payload("Settings load failed", error),
    };
    match settings.active_account() {
        Some(account) => apply_result(apply_account_to_codex(account)),
        None => failed_message("No active account is available"),
    }
}

#[tauri::command]
pub fn clear_api_mode() -> CommandResult<CodexApplyResult> {
    apply_result(clear_codex_manager_config())
}

#[tauri::command]
pub fn restore_backup() -> CommandResult<CodexApplyResult> {
    apply_result(restore_latest_backup())
}

#[tauri::command]
pub fn read_restore_preview() -> CommandResult<BackupConfigPreview> {
    match read_latest_backup_preview() {
        Ok(payload) => ok("恢复预览已加载", payload),
        Err(error) => failed_payload("恢复预览加载失败", error),
    }
}

#[tauri::command]
pub fn read_codex_config() -> CommandResult<CodexConfigView> {
    match read_codex_view() {
        Ok(payload) => ok("Codex config loaded", payload),
        Err(error) => failed_payload("Codex config load failed", error),
    }
}

#[tauri::command]
pub fn open_codex_install_page() -> CommandResult<serde_json::Value> {
    let url = install_url();
    match open_external_url(url) {
        Ok(()) => ok("Codex 安装页面已打开", serde_json::json!({})),
        Err(error) => failed_payload("打开 Codex 安装页面失败", error),
    }
}

#[tauri::command]
pub fn detect_codex_path() -> CommandResult<CodexPathInfo> {
    let saved = SettingsStore::default()
        .load()
        .map(|settings| settings.codex_app_path)
        .unwrap_or_default();
    match resolve_codex_path(Some(&saved)) {
        Ok(payload) => ok("Codex 安装位置已检测", payload),
        Err(error) => failed_payload("Codex 安装位置检测失败", error),
    }
}

#[tauri::command]
pub fn launch_codex(request: LaunchRequest) -> CommandResult<serde_json::Value> {
    match launch_codex_core(request) {
        Ok(()) => ok("Codex 启动成功", serde_json::json!({})),
        Err(error) => failed_payload("Codex launch failed", error),
    }
}

#[tauri::command]
pub fn restart_codex(request: LaunchRequest) -> CommandResult<serde_json::Value> {
    match restart_codex_core(request) {
        Ok(()) => ok("Codex 重启成功", serde_json::json!({})),
        Err(error) => failed_payload("Codex restart failed", error),
    }
}

fn install_url() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "https://codexapp.agentsmirror.com/latest/mac-arm64"
        } else {
            "https://codexapp.agentsmirror.com/latest/mac-intel"
        }
    } else {
        "https://codexapp.agentsmirror.com/latest/win"
    }
}

fn open_external_url(url: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!("Start-Process '{}'", url.replace('\'', "''")),
        ]);
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("failed to open URL: {error}"))
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("failed to open URL: {error}"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("failed to open URL: {error}"))
    }
}

fn select_login_auth(settings: &AppSettings, login_id: &str) -> AuthState {
    if let Some(account) = settings.login_account(login_id) {
        account.auth.clone()
    } else if let Some(account) = settings.active_login_account() {
        account.auth.clone()
    } else {
        settings.auth.clone()
    }
}

fn settings_payload(message: &str) -> CommandResult<SettingsPayload> {
    let store = SettingsStore::default();
    match store.load() {
        Ok(settings) => ok(
            message,
            SettingsPayload {
                settings,
                settings_path: store.path().to_string_lossy().to_string(),
            },
        ),
        Err(error) => failed_payload("Settings load failed", error),
    }
}

fn apply_result(result: anyhow::Result<CodexApplyResult>) -> CommandResult<CodexApplyResult> {
    match result {
        Ok(payload) => ok(&payload.message.clone(), payload),
        Err(error) => failed_payload("Codex apply failed", error),
    }
}

fn ok<T: Serialize>(message: &str, payload: T) -> CommandResult<T> {
    CommandResult {
        status: "ok".to_string(),
        message: message.to_string(),
        payload,
    }
}

fn failed_message(message: &str) -> CommandResult<CodexApplyResult> {
    failed(
        message,
        CodexApplyResult {
            status: "failed".to_string(),
            message: message.to_string(),
            config_path: String::new(),
            auth_path: String::new(),
            backup_path: String::new(),
            account_id: String::new(),
            account_name: String::new(),
            base_url: String::new(),
        },
    )
}

fn failed_payload<T: Serialize>(message: &str, error: impl std::fmt::Display) -> CommandResult<T>
where
    T: Default,
{
    failed(&format!("{message}: {error}"), T::default())
}

fn failed<T: Serialize>(message: &str, payload: T) -> CommandResult<T> {
    CommandResult {
        status: "failed".to_string(),
        message: message.to_string(),
        payload,
    }
}
