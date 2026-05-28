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

    let mut args = Vec::new();
    if settings.plugin_enabled {
        for arg in plugin_unlock_arguments(
            debug_port.expect("debug port exists when plugin unlock is enabled"),
        ) {
            args.push(arg);
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
            args.push(arg.to_string());
        }
    }
    #[cfg(windows)]
    if !path_info.app_user_model_id.is_empty() {
        activate_packaged_app(&path_info.app_user_model_id, &command_line_arguments(&args))
            .with_context(|| {
                format!(
                    "failed to activate Codex package {}",
                    path_info.app_user_model_id
                )
            })?;
        if let Some(debug_port) = debug_port {
            spawn_plugin_unlock_injection(debug_port);
        }
        return Ok(());
    }

    let mut command = Command::new(&path_info.executable_path);
    command.args(&args);
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
    terminate_codex_processes_by_name()
}

#[cfg(not(windows))]
fn stop_codex_processes() -> anyhow::Result<()> {
    Ok(())
}

fn command_line_arguments(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote_windows_argument(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_windows_argument(arg: &str) -> String {
    if !arg.is_empty() && !arg.bytes().any(|byte| matches!(byte, b' ' | b'\t' | b'"')) {
        return arg.to_string();
    }
    let mut output = String::from("\"");
    let mut backslashes = 0;
    for ch in arg.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                output.push_str(&"\\".repeat(backslashes * 2 + 1));
                output.push('"');
                backslashes = 0;
            }
            _ => {
                output.push_str(&"\\".repeat(backslashes));
                output.push(ch);
                backslashes = 0;
            }
        }
    }
    output.push_str(&"\\".repeat(backslashes * 2));
    output.push('"');
    output
}

#[cfg(windows)]
fn activate_packaged_app(app_user_model_id: &str, arguments: &str) -> anyhow::Result<u32> {
    use windows::Win32::System::Com::{
        CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    };
    use windows::Win32::UI::Shell::{ApplicationActivationManager, IApplicationActivationManager};
    use windows::core::HSTRING;

    unsafe {
        let coinit = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let should_uninitialize = coinit.is_ok();
        coinit.ok().or_else(|error| {
            const RPC_E_CHANGED_MODE: i32 = -2147417850;
            if error.code().0 == RPC_E_CHANGED_MODE {
                Ok(())
            } else {
                Err(error)
            }
        })?;

        let result: windows::core::Result<u32> = (|| {
            let manager: IApplicationActivationManager =
                CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)?;
            let process_id = manager.ActivateApplication(
                &HSTRING::from(app_user_model_id),
                &HSTRING::from(arguments),
                windows::Win32::UI::Shell::ACTIVATEOPTIONS(0),
            )?;
            Ok(process_id)
        })();

        if should_uninitialize {
            CoUninitialize();
        }
        result.map_err(Into::into)
    }
}

#[cfg(windows)]
fn terminate_codex_processes_by_name() -> anyhow::Result<()> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, TerminateProcess,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let snapshot_guard = HandleGuard(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot_guard.0, &mut entry).is_err() {
            return Ok(());
        }
        loop {
            let name = process_entry_name(&entry);
            if name.eq_ignore_ascii_case("Codex.exe") || name.eq_ignore_ascii_case("codex.exe") {
                if let Ok(handle) = OpenProcess(
                    PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    entry.th32ProcessID,
                ) {
                    let process_guard = HandleGuard(handle);
                    let _ = TerminateProcess(process_guard.0, 1);
                }
            }
            if Process32NextW(snapshot_guard.0, &mut entry).is_err() {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
struct HandleGuard(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn process_entry_name(
    entry: &windows::Win32::System::Diagnostics::ToolHelp::PROCESSENTRY32W,
) -> String {
    use std::os::windows::ffi::OsStringExt;

    let end = entry
        .szExeFile
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(entry.szExeFile.len());
    std::ffi::OsString::from_wide(&entry.szExeFile[..end])
        .to_string_lossy()
        .to_string()
}
