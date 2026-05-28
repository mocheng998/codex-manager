use std::path::PathBuf;

use directories::{BaseDirs, ProjectDirs};

const QUALIFIER: &str = "dev";
const ORGANIZATION: &str = "CodexManager";
const APPLICATION: &str = "CodexManager";

pub fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
}

pub fn config_dir() -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.config_dir().to_path_buf())
        .unwrap_or_else(|| fallback_home_dir().join(".codex-manager"))
}

pub fn data_dir() -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| fallback_home_dir().join(".codex-manager"))
}

pub fn settings_file() -> PathBuf {
    config_dir().join("config.json")
}

pub fn backup_dir() -> PathBuf {
    data_dir().join("backups")
}

pub fn codex_home_dir() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| fallback_home_dir().join(".codex"))
}

pub fn codex_config_file() -> PathBuf {
    codex_home_dir().join("config.toml")
}

pub fn codex_auth_file() -> PathBuf {
    codex_home_dir().join("auth.json")
}

fn fallback_home_dir() -> PathBuf {
    BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}
