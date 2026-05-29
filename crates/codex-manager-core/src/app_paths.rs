use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::bail;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPathInfo {
    pub app_dir: String,
    pub executable_path: String,
    pub version: String,
    pub source: String,
    pub app_user_model_id: String,
}

pub fn resolve_codex_path(saved_app_path: Option<&str>) -> anyhow::Result<CodexPathInfo> {
    if let Some(saved) = saved_app_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(info) = normalize_codex_app_path(Path::new(saved), "saved") {
            return Ok(info);
        }
    }

    if let Some(env_path) = std::env::var_os("CODEX_APP_PATH")
        .map(PathBuf::from)
        .and_then(|path| normalize_codex_app_path(&path, "CODEX_APP_PATH"))
    {
        return Ok(env_path);
    }

    #[cfg(windows)]
    {
        if let Some(info) = find_windows_package_manager_codex() {
            return Ok(info);
        }
        if let Some(info) = find_windows_appmodel_runtime_codex() {
            return Ok(info);
        }
        if let Some(info) = find_windows_registry_codex() {
            return Ok(info);
        }
        if let Some(info) = find_latest_codex_windows_app_dir_default() {
            return Ok(info);
        }
        if let Some(info) = find_windows_common_install() {
            return Ok(info);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(info) = find_macos_codex_app_default() {
            return Ok(info);
        }
    }

    bail!("Codex executable was not found")
}

pub fn normalize_codex_app_path(path: &Path, source: &str) -> Option<CodexPathInfo> {
    if path.as_os_str().is_empty() {
        return None;
    }

    let file_name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
    if file_name.eq_ignore_ascii_case("Codex.exe") || file_name.eq_ignore_ascii_case("codex.exe") {
        let app_dir = path.parent()?.to_path_buf();
        return Some(info_from_app_dir(app_dir, path.to_path_buf(), source));
    }

    if path.extension() == Some(OsStr::new("app")) {
        let executable = path.join("Contents").join("MacOS").join("Codex");
        if executable.exists() {
            return Some(info_from_app_dir(path.to_path_buf(), executable, source));
        }
    }

    if path.is_file() {
        let app_dir = path.parent()?.to_path_buf();
        return Some(info_from_app_dir(app_dir, path.to_path_buf(), source));
    }

    let upper = path.join("Codex.exe");
    if upper.exists() {
        return Some(info_from_app_dir(path.to_path_buf(), upper, source));
    }
    let lower = path.join("codex.exe");
    if lower.exists() {
        return Some(info_from_app_dir(path.to_path_buf(), lower, source));
    }

    let nested_app = path.join("app");
    let upper = nested_app.join("Codex.exe");
    if upper.exists() {
        return Some(info_from_app_dir(nested_app, upper, source));
    }
    let lower = nested_app.join("codex.exe");
    if lower.exists() {
        return Some(info_from_app_dir(nested_app, lower, source));
    }

    if let Some(app_dir) = packaged_app_dir_from_path(path) {
        return Some(info_from_packaged_app_dir(app_dir, source));
    }

    None
}

fn info_from_app_dir(app_dir: PathBuf, executable_path: PathBuf, source: &str) -> CodexPathInfo {
    let app_user_model_id = packaged_app_user_model_id(&app_dir).unwrap_or_default();
    CodexPathInfo {
        version: codex_app_version(&app_dir).unwrap_or_default(),
        app_dir: app_dir.to_string_lossy().to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        source: source.to_string(),
        app_user_model_id,
    }
}

fn info_from_packaged_app_dir(app_dir: PathBuf, source: &str) -> CodexPathInfo {
    let executable_path = packaged_codex_executable_path(&app_dir);
    info_from_app_dir(app_dir, executable_path, source)
}

fn packaged_app_dir_from_path(path: &Path) -> Option<PathBuf> {
    let package_name = package_name_from_app_dir(path)?;
    if !is_openai_codex_package_name(&package_name) {
        return None;
    }
    if path
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.eq_ignore_ascii_case("app"))
    {
        return Some(path.to_path_buf());
    }
    Some(path.join("app"))
}

fn packaged_codex_executable_path(app_dir: &Path) -> PathBuf {
    let upper = app_dir.join("Codex.exe");
    if upper.exists() {
        return upper;
    }
    let lower = app_dir.join("codex.exe");
    if lower.exists() {
        return lower;
    }
    upper
}

pub fn packaged_app_user_model_id(app_dir: &Path) -> Option<String> {
    let package_name = package_name_from_app_dir(app_dir)?;
    if !is_openai_codex_package_name(&package_name) {
        return None;
    }
    let identity_name = package_name.split_once('_')?.0;
    let publisher_id = package_name.rsplit_once("__")?.1;
    if publisher_id.is_empty() {
        return None;
    }
    Some(format!("{identity_name}_{publisher_id}!App"))
}

fn package_name_from_app_dir(app_dir: &Path) -> Option<String> {
    let path = app_dir.to_string_lossy().replace('\\', "/");
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let mut package_name = parts.next_back()?;
    if package_name.eq_ignore_ascii_case("app") {
        package_name = parts.next_back()?;
    }
    Some(package_name.to_string())
}

fn is_openai_codex_package_name(name: &str) -> bool {
    name.starts_with("OpenAI.Codex_") && name.contains("__")
}

fn codex_app_version(app_dir: &Path) -> Option<String> {
    if app_dir.extension() == Some(OsStr::new("app")) {
        return macos_app_version(app_dir);
    }
    let package_dir = if app_dir
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.eq_ignore_ascii_case("app"))
    {
        app_dir.parent()?
    } else {
        app_dir
    };
    codex_package_version(package_dir)
}

fn codex_package_version(package_dir: &Path) -> Option<String> {
    let path = package_dir.to_string_lossy().replace('\\', "/");
    let name = path
        .split('/')
        .rev()
        .find(|part| part.starts_with("OpenAI.Codex_"))?;
    let rest = name.strip_prefix("OpenAI.Codex_")?;
    let version = rest.split_once('_')?.0;
    (!version.is_empty()).then(|| version.to_string())
}

#[cfg(windows)]
fn find_windows_package_manager_codex() -> Option<CodexPathInfo> {
    let manager = windows::Management::Deployment::PackageManager::new().ok()?;
    let packages = manager.FindPackages().ok()?;
    let mut best: Option<(Vec<u32>, CodexPathInfo)> = None;

    for package in packages {
        let Some(id) = package.Id().ok() else {
            continue;
        };
        let Some(name) = id.Name().ok().map(|value| value.to_string()) else {
            continue;
        };
        if name != "OpenAI.Codex" {
            continue;
        }

        let Some(install_path) = package_installed_path(&package) else {
            continue;
        };
        let Some(mut info) = normalize_codex_app_path(Path::new(&install_path), "PackageManager")
        else {
            continue;
        };
        if info.version.is_empty() {
            if let Ok(version) = id.Version() {
                info.version = package_version_string(version);
            }
        }

        let version = id
            .Version()
            .ok()
            .map(package_version_tuple)
            .or_else(|| {
                version_tuple(
                    Path::new(&info.app_dir)
                        .parent()
                        .unwrap_or(Path::new(&info.app_dir)),
                )
            })
            .unwrap_or_default();

        if best
            .as_ref()
            .is_none_or(|(best_version, _)| version > *best_version)
        {
            best = Some((version, info));
        }
    }

    best.map(|(_, info)| info)
}

#[cfg(windows)]
fn find_windows_appmodel_runtime_codex() -> Option<CodexPathInfo> {
    use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
    use windows::Win32::Storage::Packaging::Appx::GetPackagesByPackageFamily;
    use windows::core::{PCWSTR, PWSTR};

    const CODEX_PACKAGE_FAMILY: &str = "OpenAI.Codex_2p2nqsd0c76g0";

    unsafe {
        let family = wide_null(CODEX_PACKAGE_FAMILY);
        let mut count = 0_u32;
        let mut buffer_length = 0_u32;
        let first = GetPackagesByPackageFamily(
            PCWSTR(family.as_ptr()),
            &mut count,
            None,
            &mut buffer_length,
            PWSTR::null(),
        );
        if first != ERROR_INSUFFICIENT_BUFFER && first != ERROR_SUCCESS {
            return None;
        }
        if count == 0 {
            return None;
        }

        let mut package_name_buffer = vec![0_u16; buffer_length as usize];
        let mut package_name_ptrs = vec![PWSTR::null(); count as usize];
        let second = GetPackagesByPackageFamily(
            PCWSTR(family.as_ptr()),
            &mut count,
            Some(package_name_ptrs.as_mut_ptr()),
            &mut buffer_length,
            PWSTR(package_name_buffer.as_mut_ptr()),
        );
        if second != ERROR_SUCCESS {
            return None;
        }

        package_name_ptrs
            .into_iter()
            .filter_map(|name| pwstr_to_string(name))
            .filter_map(|full_name| package_path_by_full_name(&full_name))
            .filter_map(|path| normalize_codex_app_path(Path::new(&path), "AppModelRuntime"))
            .max_by(|left, right| {
                version_tuple(
                    Path::new(&left.app_dir)
                        .parent()
                        .unwrap_or(Path::new(&left.app_dir)),
                )
                .cmp(&version_tuple(
                    Path::new(&right.app_dir)
                        .parent()
                        .unwrap_or(Path::new(&right.app_dir)),
                ))
            })
    }
}

#[cfg(windows)]
fn package_path_by_full_name(full_name: &str) -> Option<String> {
    use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
    use windows::Win32::Storage::Packaging::Appx::GetPackagePathByFullName;
    use windows::core::{PCWSTR, PWSTR};

    unsafe {
        let full_name = wide_null(full_name);
        let mut path_length = 0_u32;
        let first =
            GetPackagePathByFullName(PCWSTR(full_name.as_ptr()), &mut path_length, PWSTR::null());
        if first != ERROR_INSUFFICIENT_BUFFER && first != ERROR_SUCCESS {
            return None;
        }
        if path_length == 0 {
            return None;
        }
        let mut path = vec![0_u16; path_length as usize];
        let second = GetPackagePathByFullName(
            PCWSTR(full_name.as_ptr()),
            &mut path_length,
            PWSTR(path.as_mut_ptr()),
        );
        if second != ERROR_SUCCESS {
            return None;
        }
        wide_slice_to_string(&path)
    }
}

#[cfg(windows)]
fn find_windows_registry_codex() -> Option<CodexPathInfo> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, KEY_READ};

    const PACKAGES_KEY: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";

    enumerate_registry_subkeys(HKEY_CURRENT_USER, PACKAGES_KEY, KEY_READ)
        .into_iter()
        .filter(|name| is_openai_codex_package_name(name))
        .filter_map(|name| {
            let subkey = format!(r"{PACKAGES_KEY}\{name}");
            read_registry_string(HKEY_CURRENT_USER, &subkey, "PackageRootFolder")
        })
        .filter_map(|path| normalize_codex_app_path(Path::new(&path), "Registry"))
        .max_by(|left, right| {
            version_tuple(
                Path::new(&left.app_dir)
                    .parent()
                    .unwrap_or(Path::new(&left.app_dir)),
            )
            .cmp(&version_tuple(
                Path::new(&right.app_dir)
                    .parent()
                    .unwrap_or(Path::new(&right.app_dir)),
            ))
        })
}

#[cfg(windows)]
fn enumerate_registry_subkeys(
    root: windows::Win32::System::Registry::HKEY,
    subkey: &str,
    access: windows::Win32::System::Registry::REG_SAM_FLAGS,
) -> Vec<String> {
    use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{HKEY, RegEnumKeyExW, RegOpenKeyExW};
    use windows::core::{PCWSTR, PWSTR};

    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            root,
            PCWSTR(wide_null(subkey).as_ptr()),
            0,
            access,
            &mut key,
        ) != ERROR_SUCCESS
        {
            return Vec::new();
        }
        let _guard = RegistryKeyGuard(key);
        let mut names = Vec::new();
        let mut index = 0_u32;
        loop {
            let mut buffer = vec![0_u16; 512];
            let mut len = buffer.len() as u32;
            let result = RegEnumKeyExW(
                key,
                index,
                PWSTR(buffer.as_mut_ptr()),
                &mut len,
                None,
                PWSTR::null(),
                None,
                None,
            );
            if result == ERROR_NO_MORE_ITEMS {
                break;
            }
            if result == ERROR_SUCCESS {
                if let Some(name) = wide_slice_to_string(&buffer[..len as usize]) {
                    names.push(name);
                }
            }
            index += 1;
        }
        names
    }
}

#[cfg(windows)]
fn read_registry_string(
    root: windows::Win32::System::Registry::HKEY,
    subkey: &str,
    name: &str,
) -> Option<String> {
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        HKEY, KEY_READ, REG_SZ, RegOpenKeyExW, RegQueryValueExW,
    };
    use windows::core::PCWSTR;

    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            root,
            PCWSTR(wide_null(subkey).as_ptr()),
            0,
            KEY_READ,
            &mut key,
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let _guard = RegistryKeyGuard(key);
        let mut value_type = REG_SZ;
        let mut byte_len = 0_u32;
        if RegQueryValueExW(
            key,
            PCWSTR(wide_null(name).as_ptr()),
            None,
            Some(&mut value_type),
            None,
            Some(&mut byte_len),
        ) != ERROR_SUCCESS
        {
            return None;
        }
        if byte_len == 0 {
            return None;
        }
        let mut buffer = vec![0_u16; (byte_len as usize).div_ceil(2)];
        if RegQueryValueExW(
            key,
            PCWSTR(wide_null(name).as_ptr()),
            None,
            Some(&mut value_type),
            Some(buffer.as_mut_ptr().cast::<u8>()),
            Some(&mut byte_len),
        ) != ERROR_SUCCESS
        {
            return None;
        }
        wide_slice_to_string(&buffer)
    }
}

#[cfg(windows)]
fn wide_null(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn pwstr_to_string(value: windows::core::PWSTR) -> Option<String> {
    if value.is_null() {
        return None;
    }
    unsafe {
        let mut len = 0_usize;
        while *value.0.add(len) != 0 {
            len += 1;
        }
        wide_slice_to_string(std::slice::from_raw_parts(value.0, len))
    }
}

#[cfg(windows)]
fn wide_slice_to_string(value: &[u16]) -> Option<String> {
    use std::os::windows::ffi::OsStringExt;
    let len = value.iter().position(|ch| *ch == 0).unwrap_or(value.len());
    if len == 0 {
        return None;
    }
    Some(
        std::ffi::OsString::from_wide(&value[..len])
            .to_string_lossy()
            .to_string(),
    )
}

#[cfg(windows)]
struct RegistryKeyGuard(windows::Win32::System::Registry::HKEY);

#[cfg(windows)]
impl Drop for RegistryKeyGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::Registry::RegCloseKey(self.0);
        }
    }
}

#[cfg(windows)]
fn package_installed_path(package: &windows::ApplicationModel::Package) -> Option<String> {
    package
        .InstalledPath()
        .ok()
        .map(|path| path.to_string())
        .filter(|path| !path.trim().is_empty())
        .or_else(|| {
            package
                .InstalledLocation()
                .ok()?
                .Path()
                .ok()
                .map(|path| path.to_string())
                .filter(|path| !path.trim().is_empty())
        })
}

#[cfg(windows)]
fn package_version_tuple(version: windows::ApplicationModel::PackageVersion) -> Vec<u32> {
    vec![
        version.Major as u32,
        version.Minor as u32,
        version.Build as u32,
        version.Revision as u32,
    ]
}

#[cfg(windows)]
fn package_version_string(version: windows::ApplicationModel::PackageVersion) -> String {
    format!(
        "{}.{}.{}.{}",
        version.Major, version.Minor, version.Build, version.Revision
    )
}

#[cfg(windows)]
fn find_latest_codex_windows_app_dir_default() -> Option<CodexPathInfo> {
    find_latest_codex_app_dir_from_roots(&windows_app_package_roots())
}

#[cfg(windows)]
fn find_latest_codex_app_dir_from_roots(roots: &[PathBuf]) -> Option<CodexPathInfo> {
    roots
        .iter()
        .filter_map(|root| find_latest_codex_app_dir(root))
        .max_by(|left, right| {
            version_tuple(
                Path::new(&left.app_dir)
                    .parent()
                    .unwrap_or(Path::new(&left.app_dir)),
            )
            .cmp(&version_tuple(
                Path::new(&right.app_dir)
                    .parent()
                    .unwrap_or(Path::new(&right.app_dir)),
            ))
        })
}

#[cfg(windows)]
fn find_latest_codex_app_dir(root: &Path) -> Option<CodexPathInfo> {
    let mut matches = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| version_tuple(&path).map(|version| (version, path)))
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.0.cmp(&right.0));
    let (_, latest) = matches.pop()?;
    normalize_codex_app_path(&latest, "WindowsApps")
}

#[cfg(windows)]
fn windows_app_package_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        roots.push(PathBuf::from(program_files).join("WindowsApps"));
    }
    if let Some(program_files) = std::env::var_os("ProgramW6432") {
        roots.push(PathBuf::from(program_files).join("WindowsApps"));
    }
    roots.push(PathBuf::from(r"C:\Program Files\WindowsApps"));
    roots.sort();
    roots.dedup();
    roots
}

#[cfg(windows)]
fn find_windows_common_install() -> Option<CodexPathInfo> {
    windows_common_candidates()
        .into_iter()
        .find_map(|path| normalize_codex_app_path(&path, "common"))
}

#[cfg(windows)]
fn windows_common_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for key in [
        "LOCALAPPDATA",
        "PROGRAMFILES",
        "ProgramFiles",
        "ProgramW6432",
    ] {
        if let Some(base) = std::env::var_os(key).map(PathBuf::from) {
            candidates.push(base.join("Codex"));
            candidates.push(base.join("OpenAI").join("Codex"));
            candidates.push(base.join("OpenAI.Codex"));
            candidates.push(base.join("Programs").join("Codex"));
            candidates.push(base.join("Programs").join("OpenAI Codex"));
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

#[cfg(target_os = "macos")]
fn find_macos_codex_app_default() -> Option<CodexPathInfo> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
        roots.push(home.join("Applications"));
    }
    roots
        .into_iter()
        .flat_map(|root| macos_app_candidates(&root))
        .find_map(|path| normalize_codex_app_path(&path, "Applications"))
}

#[cfg(target_os = "macos")]
fn macos_app_candidates(root: &Path) -> Vec<PathBuf> {
    if root.extension() == Some(OsStr::new("app")) {
        return vec![root.to_path_buf()];
    }
    ["Codex.app", "OpenAI Codex.app", "OpenAI.Codex.app"]
        .into_iter()
        .map(|name| root.join(name))
        .collect()
}

fn macos_app_version(app_dir: &Path) -> Option<String> {
    let plist = std::fs::read_to_string(app_dir.join("Contents").join("Info.plist")).ok()?;
    plist_string_value(&plist, "CFBundleShortVersionString")
        .or_else(|| plist_string_value(&plist, "CFBundleVersion"))
}

fn plist_string_value(plist: &str, key: &str) -> Option<String> {
    let (_, after_key) = plist.split_once(&format!("<key>{key}</key>"))?;
    let (_, after_string_open) = after_key.split_once("<string>")?;
    let (value, _) = after_string_open.split_once("</string>")?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(windows)]
fn version_tuple(path: &Path) -> Option<Vec<u32>> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix("OpenAI.Codex_")?;
    let version = rest.split_once('_')?.0;
    let parts = version
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (!parts.is_empty()).then_some(parts)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{normalize_codex_app_path, packaged_app_user_model_id};

    #[test]
    fn normalizes_packaged_codex_directory_without_reading_executable() {
        let root = Path::new(
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.519.5221.0_x64__2p2nqsd0c76g0",
        );

        let info = normalize_codex_app_path(root, "test").expect("package path should resolve");

        assert!(
            info.app_dir
                .ends_with(r"OpenAI.Codex_26.519.5221.0_x64__2p2nqsd0c76g0\app")
        );
        assert!(
            info.executable_path
                .ends_with(r"OpenAI.Codex_26.519.5221.0_x64__2p2nqsd0c76g0\app\Codex.exe")
        );
        assert_eq!(info.version, "26.519.5221.0");
        assert_eq!(info.app_user_model_id, "OpenAI.Codex_2p2nqsd0c76g0!App");
    }

    #[test]
    fn builds_packaged_activation_id_from_nested_app_directory() {
        let app_dir = Path::new(
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.519.5221.0_x64__2p2nqsd0c76g0\app",
        );

        assert_eq!(
            packaged_app_user_model_id(app_dir).as_deref(),
            Some("OpenAI.Codex_2p2nqsd0c76g0!App")
        );
    }
}
