#![cfg_attr(windows, windows_subsystem = "windows")]

use anyhow::Result;
use codex_manager_core::{LaunchRequest, SettingsStore, launch_codex};

fn main() -> Result<()> {
    let settings = SettingsStore::default().load().unwrap_or_default();
    launch_codex(LaunchRequest {
        codex_app_path: settings.codex_app_path,
        extra_args: parse_extra_args(std::env::args().skip(1)),
    })
}

fn parse_extra_args<I, S>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_extra_args() {
        assert_eq!(
            parse_extra_args(["--foo", "bar"]),
            vec!["--foo".to_string(), "bar".to_string()]
        );
    }
}
