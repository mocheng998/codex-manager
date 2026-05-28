use std::process::Command;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::app_paths::resolve_codex_path;
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

    let requested_path = if request.codex_app_path.trim().is_empty() {
        settings.codex_app_path
    } else {
        request.codex_app_path
    };
    let path_info = resolve_codex_path(Some(&requested_path))
        .context("Codex app path is not configured and auto detection failed")?;

    if restart {
        stop_codex_processes()?;
    }

    let debug_port = if settings.plugin_enabled {
        Some(select_free_local_port().context("failed to select Codex debug port")?)
    } else {
        None
    };

    let mut command = Command::new(&path_info.executable_path);
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
    command
        .spawn()
        .with_context(|| format!("failed to launch Codex at {}", path_info.executable_path))?;
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
