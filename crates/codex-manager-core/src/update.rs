use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

pub const DEFAULT_REPOSITORY: &str = "mocheng998/codex-manager";
pub const DEFAULT_LATEST_JSON_URL: &str =
    "https://github.com/mocheng998/codex-manager/releases/latest/download/latest.json";
pub const DEFAULT_GITHUB_RELEASE_URL: &str =
    "https://api.github.com/repos/mocheng998/codex-manager/releases/latest";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_summary: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub body: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
}

pub fn check_for_update(current_version: &str) -> anyhow::Result<UpdateCheck> {
    check_for_update_with_url(current_version, "")
}

pub fn check_for_update_with_url(
    current_version: &str,
    update_manifest_url: &str,
) -> anyhow::Result<UpdateCheck> {
    let custom_url = update_manifest_url.trim();
    if !custom_url.is_empty() {
        let release = fetch_release_from_url(custom_url)?;
        return update_check_from_release(current_version, release);
    }

    let release = fetch_latest_release().or_else(|latest_json_error| {
        fetch_github_latest_release().map_err(|github_error| {
            anyhow::anyhow!("latest.json: {latest_json_error}; GitHub API: {github_error}")
        })
    })?;
    update_check_from_release(current_version, release)
}

fn update_check_from_release(
    current_version: &str,
    release: Release,
) -> anyhow::Result<UpdateCheck> {
    let update_available = is_newer_version(&release.version, current_version)?;
    Ok(UpdateCheck {
        current_version: current_version.to_string(),
        latest_version: Some(release.version),
        release_summary: release.body,
        asset_name: release.asset_name,
        asset_url: release.asset_url,
        update_available,
    })
}

pub fn fetch_release_from_url(url: &str) -> anyhow::Result<Release> {
    let payload = client()?
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    release_from_latest_json_payload(&payload)
}

pub fn fetch_latest_release() -> anyhow::Result<Release> {
    let payload = client()?
        .get(DEFAULT_LATEST_JSON_URL)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    release_from_latest_json_payload(&payload)
}

pub fn fetch_github_latest_release() -> anyhow::Result<Release> {
    let payload = client()?
        .get(DEFAULT_GITHUB_RELEASE_URL)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    release_from_github_payload(&payload)
}

pub fn release_from_latest_json_payload(payload: &Value) -> anyhow::Result<Release> {
    let version = payload
        .get("version")
        .or_else(|| payload.get("tag_name"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("latest.json missing version"))?
        .to_string();
    let assets = payload
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|asset| {
            let name = asset.get("name")?.as_str()?.to_string();
            let url = asset
                .get("url")
                .or_else(|| asset.get("browser_download_url"))?
                .as_str()?
                .to_string();
            Some((name, url))
        })
        .collect::<Vec<_>>();
    let selected = select_update_asset(&assets);
    Ok(Release {
        version,
        url: payload
            .get("url")
            .or_else(|| payload.get("html_url"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        body: payload
            .get("body")
            .or_else(|| payload.get("release_summary"))
            .or_else(|| payload.get("notes"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset_name: selected.as_ref().map(|asset| asset.0.clone()),
        asset_url: selected.map(|asset| asset.1),
    })
}

pub fn release_from_github_payload(payload: &Value) -> anyhow::Result<Release> {
    let version = payload
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("release payload missing tag_name"))?
        .to_string();
    let assets = payload
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|asset| {
            Some((
                asset.get("name")?.as_str()?.to_string(),
                asset.get("browser_download_url")?.as_str()?.to_string(),
            ))
        })
        .collect::<Vec<_>>();
    let selected = select_update_asset(&assets);
    Ok(Release {
        version,
        url: payload
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        body: payload
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset_name: selected.as_ref().map(|asset| asset.0.clone()),
        asset_url: selected.map(|asset| asset.1),
    })
}

pub fn is_newer_version(candidate: &str, current: &str) -> anyhow::Result<bool> {
    let mut left = parse_version_tag(candidate)?;
    let mut right = parse_version_tag(current)?;
    let len = left.len().max(right.len());
    left.resize(len, 0);
    right.resize(len, 0);
    Ok(left > right)
}

pub fn parse_version_tag(value: &str) -> anyhow::Result<Vec<u64>> {
    let normalized = value.trim().trim_start_matches(['v', 'V']);
    let mut digits = String::new();
    for ch in normalized.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            digits.push(ch);
        } else {
            break;
        }
    }
    if digits.is_empty() {
        anyhow::bail!("Invalid version tag: {value}");
    }
    digits
        .split('.')
        .map(|part| part.parse::<u64>().map_err(Into::into))
        .collect()
}

pub fn select_update_asset(assets: &[(String, String)]) -> Option<(String, String)> {
    let named = assets
        .iter()
        .filter(|(name, url)| !name.trim().is_empty() && !url.trim().is_empty())
        .collect::<Vec<_>>();
    named
        .iter()
        .find(|(name, _)| platform_asset_rank(&name.to_ascii_lowercase()) == 0)
        .or_else(|| named.first())
        .map(|(name, url)| ((*name).clone(), (*url).clone()))
}

fn client() -> anyhow::Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(format!("Codex Manager/{}", env!("CARGO_PKG_VERSION")))
        .build()?)
}

fn platform_asset_rank(name: &str) -> u8 {
    if cfg!(windows) && is_windows_installer_asset(name) {
        return 0;
    }
    if cfg!(target_os = "macos") && is_macos_installer_asset(name) {
        return 0;
    }
    2
}

fn is_windows_installer_asset(name: &str) -> bool {
    name.contains("codex")
        && name.contains("manager")
        && (name.ends_with(".msi")
            || name.ends_with("-setup.exe")
            || name.ends_with("_setup.exe")
            || name.ends_with("setup.exe")
            || name.ends_with("installer.exe"))
}

fn is_macos_installer_asset(name: &str) -> bool {
    name.contains("codex") && name.contains("manager") && name.ends_with(".dmg")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        is_newer_version, parse_version_tag, release_from_latest_json_payload, select_update_asset,
    };

    #[test]
    fn parses_version_tags() {
        assert_eq!(parse_version_tag("v1.2.3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_version_tag("1.2.3-beta.1").unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn compares_versions_with_padding() {
        assert!(is_newer_version("1.0.9", "1.0.8").unwrap());
        assert!(is_newer_version("1.1", "1.0.9").unwrap());
        assert!(!is_newer_version("1.0.8", "1.0.8").unwrap());
    }

    #[test]
    fn parses_latest_json_release() {
        let release = release_from_latest_json_payload(&json!({
            "version": "v1.0.9",
            "notes": "bug fixes",
            "assets": [
                { "name": "Codex Manager_1.0.9_x64_en-US.msi", "url": "https://example.test/app.msi" }
            ]
        }))
        .unwrap();

        assert_eq!(release.version, "v1.0.9");
        assert_eq!(
            release.asset_name.as_deref(),
            Some("Codex Manager_1.0.9_x64_en-US.msi")
        );
    }

    #[test]
    fn falls_back_to_first_asset_when_platform_specific_missing() {
        let asset = select_update_asset(&[(
            "codex-manager-source.zip".to_string(),
            "https://example.test/source.zip".to_string(),
        )])
        .unwrap();

        assert_eq!(asset.0, "codex-manager-source.zip");
    }
}
