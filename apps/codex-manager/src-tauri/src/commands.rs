use codex_manager_core::{
    Account, AppSettings, BackupConfigPreview, CodexApplyResult, CodexConfigView, LaunchRequest,
    LoginCredentials, LoginPayload, RemoteKeyDecryptPayload, RemoteKeySearchPayload, SettingsStore,
    apply_account_to_codex, clear_codex_manager_config, launch_codex as launch_codex_core,
    login_new_api, read_codex_view, read_latest_backup_preview,
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
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptRemoteKeyRequest {
    #[serde(default, alias = "id")]
    pub token_id: String,
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
    settings.auth.user = None;
    settings.auth.cookies.clear();
    settings.auth.updated_at_ms = 0;
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
pub fn search_remote_keys(
    request: SearchRemoteKeysRequest,
) -> CommandResult<RemoteKeySearchPayload> {
    let settings = match SettingsStore::default().load() {
        Ok(settings) => settings,
        Err(error) => return failed_payload("Settings load failed", error),
    };
    match codex_manager_core::search_remote_keys(&settings.auth, &request.keyword) {
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
    match codex_manager_core::decrypt_remote_key(&settings.auth, &request.token_id) {
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
