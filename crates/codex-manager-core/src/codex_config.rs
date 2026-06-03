use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, Table, value};

use crate::paths;
use crate::settings::{Account, clean_url, normalize_api_key};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupSnapshot {
    created_at_ms: u128,
    reason: String,
    account: Option<Account>,
    config_toml: FileSnapshot,
    auth_json: FileSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileSnapshot {
    exists: bool,
    contents: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexApplyResult {
    pub status: String,
    pub message: String,
    pub config_path: String,
    pub auth_path: String,
    pub backup_path: String,
    pub account_id: String,
    pub account_name: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexConfigView {
    pub config_path: String,
    pub auth_path: String,
    pub config_toml: String,
    pub auth_json: String,
    pub backup_available: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupConfigPreview {
    pub backup_path: String,
    pub reason: String,
    pub created_at_ms: u128,
    pub source_account_name: String,
    pub config_path: String,
    pub auth_path: String,
    pub config_toml: String,
    pub auth_json: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPreferenceConfig {
    pub locale_override: String,
    pub developer_instructions: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPreferenceResult {
    pub config_path: String,
    pub backup_path: String,
    pub locale_override: String,
    pub developer_instructions: String,
}

pub fn apply_account_to_codex(account: &Account) -> anyhow::Result<CodexApplyResult> {
    if !account.enabled {
        bail!("account is disabled");
    }
    let api_key = normalize_api_key(&account.api_key);
    if api_key.is_empty() {
        bail!("account api key is empty");
    }
    let base_url = clean_url(&account.base_url);
    if base_url.is_empty() {
        bail!("account base url is empty");
    }

    let backup_path = create_backup("apply-account", Some(account.clone()))?;
    let config_path = paths::codex_config_file();
    let auth_path = paths::codex_auth_file();
    let current_config = read_optional(&config_path).contents;
    let next_config = build_codex_config(&current_config, &base_url);
    let next_auth = build_codex_auth(&api_key);

    atomic_write(&config_path, &next_config)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    atomic_write(&auth_path, &next_auth)
        .with_context(|| format!("failed to write {}", auth_path.display()))?;

    Ok(CodexApplyResult {
        status: "ok".to_string(),
        message: format!("Applied account {}", account.name),
        config_path: config_path.to_string_lossy().to_string(),
        auth_path: auth_path.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        account_id: account.id.clone(),
        account_name: account.name.clone(),
        base_url,
    })
}

pub fn clear_codex_manager_config() -> anyhow::Result<CodexApplyResult> {
    let backup_path = create_backup("clear-manager-config", None)?;
    let config_path = paths::codex_config_file();
    let auth_path = paths::codex_auth_file();
    let current_config = read_optional(&config_path).contents;
    let next_config = remove_manager_provider(&current_config);
    let next_auth = "{}\n".to_string();
    atomic_write(&config_path, &next_config)?;
    atomic_write(&auth_path, &next_auth)?;
    Ok(CodexApplyResult {
        status: "ok".to_string(),
        message: "Cleared Codex Manager API mode".to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        auth_path: auth_path.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        account_id: String::new(),
        account_name: String::new(),
        base_url: String::new(),
    })
}

pub fn restore_latest_backup() -> anyhow::Result<CodexApplyResult> {
    let backup_path = latest_backup_path().context("no backup is available")?;
    let snapshot = read_backup(&backup_path)?;
    write_snapshot_file(&paths::codex_config_file(), &snapshot.config_toml)?;
    write_snapshot_file(&paths::codex_auth_file(), &snapshot.auth_json)?;
    Ok(CodexApplyResult {
        status: "ok".to_string(),
        message: "Restored latest Codex configuration backup".to_string(),
        config_path: paths::codex_config_file().to_string_lossy().to_string(),
        auth_path: paths::codex_auth_file().to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        account_id: snapshot
            .account
            .as_ref()
            .map(|a| a.id.clone())
            .unwrap_or_default(),
        account_name: snapshot
            .account
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_default(),
        base_url: snapshot
            .account
            .as_ref()
            .map(|a| a.base_url.clone())
            .unwrap_or_default(),
    })
}

pub fn read_latest_backup_preview() -> anyhow::Result<BackupConfigPreview> {
    let backup_path = latest_backup_path().context("no backup is available")?;
    let snapshot = read_backup(&backup_path)?;
    Ok(BackupConfigPreview {
        backup_path: backup_path.to_string_lossy().to_string(),
        reason: snapshot.reason,
        created_at_ms: snapshot.created_at_ms,
        source_account_name: snapshot
            .account
            .as_ref()
            .map(|account| account.name.clone())
            .unwrap_or_default(),
        config_path: paths::codex_config_file().to_string_lossy().to_string(),
        auth_path: paths::codex_auth_file().to_string_lossy().to_string(),
        config_toml: snapshot.config_toml.contents,
        auth_json: snapshot.auth_json.contents,
    })
}

pub fn read_codex_view() -> anyhow::Result<CodexConfigView> {
    let config_path = paths::codex_config_file();
    let auth_path = paths::codex_auth_file();
    Ok(CodexConfigView {
        config_path: config_path.to_string_lossy().to_string(),
        auth_path: auth_path.to_string_lossy().to_string(),
        config_toml: read_optional(&config_path).contents,
        auth_json: read_optional(&auth_path).contents,
        backup_available: latest_backup_path().is_some(),
    })
}

pub fn read_codex_preferences() -> anyhow::Result<CodexPreferenceResult> {
    let config_path = paths::codex_config_file();
    let config = read_optional(&config_path).contents;
    let preferences = parse_codex_preferences(&config)?;
    Ok(CodexPreferenceResult {
        config_path: config_path.to_string_lossy().to_string(),
        backup_path: String::new(),
        locale_override: preferences.locale_override,
        developer_instructions: preferences.developer_instructions,
    })
}

pub fn save_codex_preferences(
    preferences: CodexPreferenceConfig,
) -> anyhow::Result<CodexPreferenceResult> {
    let backup_path = create_backup("save-preferences", None)?;
    let config_path = paths::codex_config_file();
    let current_config = read_optional(&config_path).contents;
    let next_config = build_preference_config(&current_config, &preferences)?;
    atomic_write(&config_path, &next_config)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    let saved = parse_codex_preferences(&next_config)?;
    Ok(CodexPreferenceResult {
        config_path: config_path.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        locale_override: saved.locale_override,
        developer_instructions: saved.developer_instructions,
    })
}

fn create_backup(reason: &str, account: Option<Account>) -> anyhow::Result<PathBuf> {
    let created_at_ms = now_ms();
    let snapshot = BackupSnapshot {
        created_at_ms,
        reason: reason.to_string(),
        account,
        config_toml: read_optional(&paths::codex_config_file()),
        auth_json: read_optional(&paths::codex_auth_file()),
    };
    let dir = paths::backup_dir();
    fs::create_dir_all(&dir)?;
    let file = dir.join(format!("{created_at_ms}-{reason}.json"));
    atomic_write(&file, &serde_json::to_string_pretty(&snapshot)?)?;
    Ok(file)
}

fn latest_backup_path() -> Option<PathBuf> {
    let dir = paths::backup_dir();
    let mut entries = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    entries.sort();
    entries.pop()
}

fn read_backup(path: &Path) -> anyhow::Result<BackupSnapshot> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn read_optional(path: &Path) -> FileSnapshot {
    match fs::read_to_string(path) {
        Ok(contents) => FileSnapshot {
            exists: true,
            contents,
        },
        Err(_) => FileSnapshot {
            exists: false,
            contents: String::new(),
        },
    }
}

fn write_snapshot_file(path: &Path, snapshot: &FileSnapshot) -> anyhow::Result<()> {
    if snapshot.exists {
        atomic_write(path, &snapshot.contents)
    } else {
        let _ = fs::remove_file(path);
        Ok(())
    }
}

fn build_codex_auth(api_key: &str) -> String {
    format!(
        "{}\n",
        serde_json::json!({ "OPENAI_API_KEY": api_key }).to_string()
    )
}

fn build_codex_config(current: &str, base_url: &str) -> String {
    let without_provider = remove_manager_provider(current);
    let mut lines = without_provider
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    upsert_top_level(&mut lines, "model_provider", "\"CodexManager\"".to_string());
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines.push(String::new());
    lines.push("[model_providers.CodexManager]".to_string());
    lines.push("name = \"CodexManager\"".to_string());
    lines.push("wire_api = \"responses\"".to_string());
    lines.push("requires_openai_auth = true".to_string());
    lines.push(format!("base_url = \"{}\"", escape_toml_string(base_url)));
    format!("{}\n", lines.join("\n").trim_end())
}

fn remove_manager_provider(current: &str) -> String {
    let lines = current.lines().collect::<Vec<_>>();
    let mut kept = Vec::new();
    let mut skipping_manager_section = false;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            skipping_manager_section = trimmed == "[model_providers.CodexManager]";
            if skipping_manager_section {
                continue;
            }
        }
        if skipping_manager_section {
            continue;
        }
        if trimmed.starts_with("model_provider") && trimmed.contains("CodexManager") {
            continue;
        }
        kept.push(line.to_string());
    }
    format!("{}\n", kept.join("\n").trim_end())
}

fn upsert_top_level(lines: &mut Vec<String>, key: &str, value: String) {
    let first_table = lines
        .iter()
        .position(|line| line.trim_start().starts_with('['))
        .unwrap_or(lines.len());
    for line in lines.iter_mut().take(first_table) {
        if line.trim_start().starts_with(&format!("{key} "))
            || line.trim_start().starts_with(&format!("{key}="))
        {
            *line = format!("{key} = {value}");
            return;
        }
    }
    lines.insert(first_table, format!("{key} = {value}"));
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_codex_preferences(config: &str) -> anyhow::Result<CodexPreferenceConfig> {
    if config.trim().is_empty() {
        return Ok(CodexPreferenceConfig::default());
    }
    let document = config.parse::<DocumentMut>()?;
    Ok(CodexPreferenceConfig {
        locale_override: document
            .get("desktop")
            .and_then(Item::as_table_like)
            .and_then(|table| table.get("localeOverride"))
            .and_then(Item::as_str)
            .unwrap_or_default()
            .to_string(),
        developer_instructions: document
            .get("developer_instructions")
            .and_then(Item::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn build_preference_config(
    current: &str,
    preferences: &CodexPreferenceConfig,
) -> anyhow::Result<String> {
    let mut document = if current.trim().is_empty() {
        DocumentMut::new()
    } else {
        current.parse::<DocumentMut>()?
    };
    let locale = preferences.locale_override.trim();
    let developer_instructions = preferences.developer_instructions.trim();

    if developer_instructions.is_empty() {
        document.remove("developer_instructions");
    } else {
        document["developer_instructions"] = value(developer_instructions);
    }

    if !document.contains_key("desktop") || !document["desktop"].is_table_like() {
        document["desktop"] = Item::Table(Table::new());
    }
    if locale.is_empty() {
        if let Some(table) = document["desktop"].as_table_like_mut() {
            table.remove("localeOverride");
        }
    } else {
        document["desktop"]["localeOverride"] = value(locale);
    }

    let output = document.to_string();
    Ok(if output.ends_with('\n') {
        output
    } else {
        format!("{output}\n")
    })
}

fn atomic_write(path: &Path, contents: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", now_ms()));
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_config_replaces_existing_manager_provider() {
        let next = build_codex_config(
            r#"model_provider = "CodexManager"

[model_providers.CodexManager]
base_url = "https://old.test/v1"
"#,
            "https://new.test/v1",
        );

        assert!(next.contains("model_provider = \"CodexManager\""));
        assert!(next.contains("base_url = \"https://new.test/v1\""));
        assert!(!next.contains("https://old.test"));
    }

    #[test]
    fn build_preference_config_updates_desktop_and_instructions() {
        let next = build_preference_config(
            r#"model_provider = "openai"

[desktop]
ambient-suggestions-enabled = true
"#,
            &CodexPreferenceConfig {
                locale_override: "zh-CN".to_string(),
                developer_instructions: "请默认使用中文回答。".to_string(),
            },
        )
        .unwrap();

        assert!(next.contains("developer_instructions = \"请默认使用中文回答。\""));
        assert!(next.contains("localeOverride = \"zh-CN\""));
        assert!(next.contains("ambient-suggestions-enabled = true"));
    }

    #[test]
    fn build_preference_config_removes_empty_values() {
        let next = build_preference_config(
            r#"developer_instructions = "old"

[desktop]
localeOverride = "zh-CN"
ambient-suggestions-enabled = true
"#,
            &CodexPreferenceConfig::default(),
        )
        .unwrap();

        assert!(!next.contains("developer_instructions"));
        assert!(!next.contains("localeOverride"));
        assert!(next.contains("ambient-suggestions-enabled = true"));
    }
}
