# Codex Manager

<p align="center">
  <a href="./resources/wechat_group.png">
    <img alt="微信交流群" src="https://img.shields.io/badge/%E5%BE%AE%E4%BF%A1-%E4%BA%A4%E6%B5%81%E7%BE%A4-green?logo=wechat&logoColor=white" />
  </a>
  <a href="./resources/wechat_personal.png">
    <img alt="个人微信" src="https://img.shields.io/badge/%E4%B8%AA%E4%BA%BA%E5%BE%AE%E4%BF%A1-%E6%B7%BB%E5%8A%A0%E5%A5%BD%E5%8F%8B-07C160?logo=wechat&logoColor=white" />
  </a>
</p>

Codex Manager 是一个基于 Rust + Tauri 的 Codex 桌面管理器，用来集中管理 Codex API Key、NewAPI 账号、配置备份和启动增强能力。

项目延续 `codex-api-keys-tweak` 的思路，但实现方式更干净：不修改 Codex 安装目录，也不 patch `app.asar`，而是通过独立的桌面管理器和启动器完成配置写入、账号切换与启动前准备。

## 核心设计

- 不 patch `app.asar`。
- 不向 Codex 安装目录写入文件。
- 使用共享 Rust Core 处理设置、备份、Codex 配置切换等逻辑。
- 使用 Tauri + React 构建可视化管理界面。
- 提供独立启动器，在启动 Codex 前自动应用当前选中的配置。

## 项目结构

```text
codex-manager/
  crates/codex-manager-core/      共享 Rust 核心逻辑
  apps/codex-manager/             Tauri + React 管理器
  apps/codex-manager-launcher/    静默启动器
  scripts/windows/                Windows 辅助脚本
```

## 数据位置

- 管理器设置：平台配置目录下的 `CodexManager/config.json`
- Codex 配置：`~/.codex/config.toml`
- Codex 认证：`~/.codex/auth.json`
- 配置备份：平台数据目录下的 `CodexManager/backups`

## 安装包

- Windows MSI 支持在安装向导中选择安装位置。
- Windows MSI 使用固定 upgrade code，安装新版本时会升级并移除旧版本，同时阻止降级安装。
- Windows MSI 使用中文 WiX 本地化文件：`apps/codex-manager/src-tauri/wix/zh-CN.wxl`。
- MSI upgrade code 固定在 `tauri.conf.json` 中，确保后续升级识别为同一个应用。

## 开发

启动前端和 Tauri 开发环境：

```powershell
cd apps/codex-manager
npm install
npm run dev
```

检查 Rust 工作区：

```powershell
cargo check --workspace
```

## 发布

当前 V1 包版本为 `1.0.25`。

```powershell
cd apps/codex-manager
npm run build:windows
```

IPC 与 NewAPI 说明见 [docs/API.md](docs/API.md)，Windows/macOS 打包说明见 [docs/PACKAGING.md](docs/PACKAGING.md)。

## 🙏 致谢

- 感谢 [Linux.do](https://linux.do/) 社区提供的反馈、测试和传播支持。
- 感谢 [Wangnov/codex-app-mirror](https://github.com/Wangnov/codex-app-mirror) 佬友提供的安装方式。
