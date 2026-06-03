# Codex Manager

Codex Manager is a Rust + Tauri rewrite of the original `codex-api-keys-tweak` idea.

The architecture follows the CodexPlusPlus route:

- no `app.asar` patching
- no writes into the Codex installation directory
- shared Rust core for settings, backup, and Codex config switching
- Tauri + React management UI
- standalone launcher entry that applies the active profile before starting Codex

## Layout

```text
codex-manager/
  crates/codex-manager-core/      Shared Rust logic
  apps/codex-manager/             Tauri + React manager
  apps/codex-manager-launcher/    Silent launcher
  scripts/windows/                Windows helper scripts
```

## Data

- Manager settings: platform config directory under `CodexManager/config.json`
- Codex config: `~/.codex/config.toml`
- Codex auth: `~/.codex/auth.json`
- Backups: platform data directory under `CodexManager/backups`

## Installer

- Windows MSI uses the WiX install directory dialog, so users can choose the install location from the setup wizard.
- Windows MSI uses a pinned upgrade code and blocks downgrades, so installing a newer package upgrades and removes the previous version.
- Windows MSI uses a Chinese WiX locale file under `apps/codex-manager/src-tauri/wix/zh-CN.wxl`.
- The MSI upgrade code is pinned in `tauri.conf.json` to keep upgrades tied to the same installed app.

## Development

```powershell
cd apps/codex-manager
npm install
npm run dev
```

```powershell
cargo check --workspace
```

## Release

V1 package version is `1.0.21`.

```powershell
cd apps/codex-manager
npm run build:windows
```

See [docs/API.md](docs/API.md) for IPC and NewAPI details, and [docs/PACKAGING.md](docs/PACKAGING.md) for Windows/macOS packaging.
