use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::codex_config::apply_account_to_codex;
use crate::plugin_unlock::{plugin_unlock_arguments, spawn_plugin_unlock_injection};
use crate::settings::SettingsStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub codex_app_path: String,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

pub fn launch_codex(request: LaunchRequest) -> anyhow::Result<()> {
    launch_codex_inner(request, false)
}

pub fn restart_codex(request: LaunchRequest) -> anyhow::Result<()> {
    launch_codex_inner(request, true)
}

fn launch_codex_inner(request: LaunchRequest, restart: bool) -> anyhow::Result<()> {
    let store = SettingsStore::default();
    let settings = store.load()?;
    if let Some(account) = settings.active_account() {
        apply_account_to_codex(account)?;
    }

    let app_path = if request.codex_app_path.trim().is_empty() {
        settings.codex_app_path
    } else {
        request.codex_app_path
    };
    let app_path = if app_path.trim().is_empty() {
        resolve_codex_executable()
            .context("Codex app path is not configured and auto detection failed")?
    } else {
        PathBuf::from(app_path.trim())
    };

    if restart {
        stop_codex_processes()?;
    }

    let debug_port = if settings.plugin_enabled {
        Some(select_free_local_port().context("failed to select Codex debug port")?)
    } else {
        None
    };

    let mut command = Command::new(app_path);
    if settings.plugin_enabled {
        for arg in plugin_unlock_arguments(
            debug_port.expect("debug port exists when plugin unlock is enabled"),
        ) {
            command.arg(arg);
        }
    }
    for arg in settings
        .launch_extra_args
        .iter()
        .chain(request.extra_args.iter())
    {
        if !arg.trim().is_empty() {
            if settings.plugin_enabled && is_remote_debug_arg(arg) {
                continue;
            }
            command.arg(arg);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.spawn().context("failed to launch Codex")?;
    if let Some(debug_port) = debug_port {
        spawn_plugin_unlock_injection(debug_port);
    }
    Ok(())
}

fn select_free_local_port() -> anyhow::Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn is_remote_debug_arg(value: &str) -> bool {
    let arg = value.trim();
    arg.starts_with("--remote-debugging-port")
        || arg.starts_with("--remote-allow-origins")
        || arg.starts_with("--remote-debugging-address")
}

fn resolve_codex_executable() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("CODEX_APP_PATH")
        .map(PathBuf::from)
        .filter(|path| path.exists())
    {
        return Ok(path);
    }

    #[cfg(windows)]
    {
        if let Some(path) = resolve_windows_store_codex()? {
            return Ok(path);
        }
    }

    bail!("Codex executable was not found")
}

#[cfg(windows)]
fn resolve_windows_store_codex() -> anyhow::Result<Option<PathBuf>> {
    let mut command = Command::new("powershell.exe");
    command.args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "$pkg = Get-AppxPackage | Where-Object { $_.Name -match 'Codex' -or $_.PackageFullName -match 'Codex' -or $_.InstallLocation -match 'Codex' } | Select-Object -First 1; if ($pkg) { $pkg.InstallLocation }",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let install_location = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if install_location.is_empty() {
        return Ok(None);
    }
    let app_dir = PathBuf::from(install_location).join("app");
    Ok(find_codex_exe(&app_dir))
}

#[cfg(windows)]
fn stop_codex_processes() -> anyhow::Result<()> {
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-WindowStyle",
        "Hidden",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        "Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -ieq 'Codex' -or $_.ProcessName -ieq 'codex' } | Stop-Process -Force -ErrorAction SilentlyContinue",
    ]);
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
    command.status().context("failed to stop Codex processes")?;
    Ok(())
}

#[cfg(not(windows))]
fn stop_codex_processes() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(windows)]
fn find_codex_exe(app_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(app_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name.to_ascii_lowercase().contains("codex") && name.ends_with(".exe") {
            return Some(path);
        }
    }
    let fallback = app_dir.join("Codex.exe");
    fallback.exists().then_some(fallback)
}
