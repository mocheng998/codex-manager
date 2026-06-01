use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::paths;

pub const DEFAULT_AUTH_BASE_URL: &str = "https://yiciyuan.one";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
}

impl Account {
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            base_url: clean_url(base_url.into()),
            api_key: normalize_api_key(api_key.into()),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: u64,
    pub username: String,
    pub display_name: String,
    pub group: String,
    pub role: i64,
    pub status: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: String,
    pub expiration_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub login_mode: String,
    pub base_url: String,
    pub user: Option<AuthUser>,
    pub cookies: Vec<StoredCookie>,
    pub updated_at_ms: u128,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            login_mode: "newApi".to_string(),
            base_url: DEFAULT_AUTH_BASE_URL.to_string(),
            user: None,
            cookies: Vec::new(),
            updated_at_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginAccount {
    pub id: String,
    pub name: String,
    pub auth: AuthState,
}

impl LoginAccount {
    pub fn from_auth(auth: AuthState) -> Self {
        let name = auth
            .user
            .as_ref()
            .map(|user| {
                let display_name = if user.display_name.trim().is_empty() {
                    user.username.trim()
                } else {
                    user.display_name.trim()
                };
                format!("{display_name} · {}", auth.base_url)
            })
            .unwrap_or_else(|| auth.base_url.clone());

        Self {
            id: Uuid::new_v4().to_string(),
            name,
            auth,
        }
    }

    pub fn matches_auth(&self, auth: &AuthState) -> bool {
        let same_base_url = clean_url(&self.auth.base_url) == clean_url(&auth.base_url);
        let same_user = self
            .auth
            .user
            .as_ref()
            .zip(auth.user.as_ref())
            .map(|(left, right)| left.id == right.id && left.username == right.username)
            .unwrap_or(false);
        same_base_url && same_user
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub codex_app_path: String,
    pub active_account_id: String,
    pub launch_extra_args: Vec<String>,
    #[serde(default)]
    pub plugin_enabled: bool,
    #[serde(default)]
    pub auth: AuthState,
    #[serde(default)]
    pub active_login_id: String,
    #[serde(default)]
    pub login_accounts: Vec<LoginAccount>,
    #[serde(default)]
    pub update_manifest_url: String,
    pub accounts: Vec<Account>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            codex_app_path: String::new(),
            active_account_id: String::new(),
            launch_extra_args: Vec::new(),
            plugin_enabled: false,
            auth: AuthState::default(),
            active_login_id: String::new(),
            login_accounts: Vec::new(),
            update_manifest_url: String::new(),
            accounts: Vec::new(),
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.auth.login_mode = if self.auth.login_mode.trim().is_empty() {
            "newApi".to_string()
        } else {
            self.auth.login_mode.trim().to_string()
        };
        self.auth.base_url = clean_url(&self.auth.base_url);
        if self.auth.base_url.is_empty() {
            self.auth.base_url = DEFAULT_AUTH_BASE_URL.to_string();
        }
        self.update_manifest_url = self.update_manifest_url.trim().to_string();
        self.login_accounts = self
            .login_accounts
            .into_iter()
            .filter_map(|mut account| {
                account.id = account.id.trim().to_string();
                if account.id.is_empty() {
                    account.id = Uuid::new_v4().to_string();
                }
                account.name = account.name.trim().to_string();
                account.auth.login_mode = if account.auth.login_mode.trim().is_empty() {
                    "newApi".to_string()
                } else {
                    account.auth.login_mode.trim().to_string()
                };
                account.auth.base_url = clean_url(&account.auth.base_url);
                if account.auth.base_url.is_empty() {
                    account.auth.base_url = DEFAULT_AUTH_BASE_URL.to_string();
                }
                let user = account.auth.user.as_ref()?;
                if account.name.is_empty() {
                    let display_name = if user.display_name.trim().is_empty() {
                        user.username.trim()
                    } else {
                        user.display_name.trim()
                    };
                    account.name = format!("{display_name} · {}", account.auth.base_url);
                }
                Some(account)
            })
            .collect();

        if self.auth.user.is_some()
            && !self
                .login_accounts
                .iter()
                .any(|account| account.matches_auth(&self.auth))
        {
            self.login_accounts
                .push(LoginAccount::from_auth(self.auth.clone()));
        }

        if !self
            .login_accounts
            .iter()
            .any(|account| account.id == self.active_login_id)
        {
            self.active_login_id = self
                .login_accounts
                .first()
                .map(|account| account.id.clone())
                .unwrap_or_default();
        }

        if let Some(active) = self.active_login_account() {
            self.auth = active.auth.clone();
        } else if self.auth.user.is_none() {
            self.auth = AuthState::default();
        }

        self.accounts = self
            .accounts
            .into_iter()
            .filter_map(|mut account| {
                account.id = account.id.trim().to_string();
                if account.id.is_empty() {
                    account.id = Uuid::new_v4().to_string();
                }
                account.name = account.name.trim().to_string();
                if account.name.is_empty() {
                    account.name = "Default API Key".to_string();
                }
                account.base_url = clean_url(account.base_url);
                account.api_key = normalize_api_key(account.api_key);
                if account.api_key.is_empty() {
                    return None;
                }
                Some(account)
            })
            .collect();

        if !self
            .accounts
            .iter()
            .any(|account| account.id == self.active_account_id && account.enabled)
        {
            self.active_account_id = self
                .accounts
                .iter()
                .find(|account| account.enabled)
                .map(|account| account.id.clone())
                .unwrap_or_default();
        }
        self
    }

    pub fn active_account(&self) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|account| account.id == self.active_account_id && account.enabled)
    }

    pub fn active_login_account(&self) -> Option<&LoginAccount> {
        self.login_accounts
            .iter()
            .find(|account| account.id == self.active_login_id)
    }

    pub fn login_account(&self, id: &str) -> Option<&LoginAccount> {
        self.login_accounts
            .iter()
            .find(|account| account.id == id.trim())
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self {
            path: paths::settings_file(),
        }
    }
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn load(&self) -> anyhow::Result<AppSettings> {
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }
        let text = fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read {}", self.path.display()))?;
        let settings: AppSettings = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse {}", self.path.display()))?;
        Ok(settings.normalized())
    }

    pub fn save(&self, settings: &AppSettings) -> anyhow::Result<AppSettings> {
        let normalized = settings.clone().normalized();
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(&normalized)?;
        fs::write(&self.path, format!("{text}\n"))
            .with_context(|| format!("failed to write {}", self.path.display()))?;
        Ok(normalized)
    }
}

pub fn clean_url(value: impl AsRef<str>) -> String {
    value.as_ref().trim().trim_end_matches('/').to_string()
}

pub fn normalize_api_key(value: impl AsRef<str>) -> String {
    let text = value.as_ref().trim();
    if text.is_empty() || text.starts_with("sk-") {
        text.to_string()
    } else {
        format!("sk-{text}")
    }
}

#[cfg(test)]
mod tests {
    use super::{AppSettings, AuthState, AuthUser, LoginAccount};

    #[test]
    fn plugin_unlock_is_disabled_by_default() {
        assert!(!AppSettings::default().plugin_enabled);
    }

    #[test]
    fn missing_plugin_unlock_setting_defaults_to_disabled() {
        let settings: AppSettings = serde_json::from_str(
            r#"{
              "codexAppPath": "",
              "activeAccountId": "",
              "launchExtraArgs": [],
              "auth": {
                "loginMode": "newApi",
                "baseUrl": "https://yiciyuan.one",
                "user": null,
                "cookies": [],
                "updatedAtMs": 0
              },
              "accounts": []
            }"#,
        )
        .expect("settings should deserialize");

        assert!(!settings.plugin_enabled);
    }

    #[test]
    fn legacy_auth_is_migrated_to_login_accounts() {
        let settings = AppSettings {
            auth: test_auth(7, "https://one.example"),
            ..AppSettings::default()
        }
        .normalized();

        assert_eq!(settings.login_accounts.len(), 1);
        assert_eq!(settings.active_login_id, settings.login_accounts[0].id);
        assert_eq!(settings.auth.base_url, "https://one.example");
        assert_eq!(settings.auth.user.as_ref().map(|user| user.id), Some(7));
    }

    #[test]
    fn active_login_account_updates_legacy_auth_view() {
        let first = LoginAccount::from_auth(test_auth(1, "https://first.example"));
        let second = LoginAccount::from_auth(test_auth(2, "https://second.example"));
        let settings = AppSettings {
            active_login_id: second.id.clone(),
            login_accounts: vec![first, second],
            ..AppSettings::default()
        }
        .normalized();

        assert_eq!(settings.auth.base_url, "https://second.example");
        assert_eq!(settings.auth.user.as_ref().map(|user| user.id), Some(2));
    }

    fn test_auth(id: u64, base_url: &str) -> AuthState {
        AuthState {
            base_url: base_url.to_string(),
            user: Some(AuthUser {
                id,
                username: format!("user{id}"),
                display_name: format!("User {id}"),
                group: "default".to_string(),
                role: 0,
                status: 1,
            }),
            ..AuthState::default()
        }
    }
}
