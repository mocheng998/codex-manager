# Codex Manager V1 打包说明

## 环境要求

- Node.js 20+
- Rust stable
- Windows 打包需要 Windows 10/11
- macOS 打包需要 macOS 13+ 和 Xcode Command Line Tools

Tauri 桌面应用通常需要在目标系统上打包。Windows 环境可以产出 Windows 安装包，macOS 安装包需要在 macOS 上执行或通过 CI 的 `macos-latest` runner 产出。

## Windows

```powershell
cd apps/codex-manager
npm install
npm run build:windows
```

输出目录：

```text
target/release/bundle/
```

常见产物包括：

- `target/release/codex-manager.exe`
- `target/release/codex-manager-launcher.exe`
- `target/release/bundle/nsis/*.exe`
- `target/release/bundle/msi/*.msi`

## macOS

```bash
cd apps/codex-manager
npm install
npm run build:mac
```

输出目录：

```text
target/release/bundle/
```

常见产物包括：

- `target/release/codex-manager`
- `target/release/codex-manager-launcher`
- `target/release/bundle/macos/*.app`
- `target/release/bundle/dmg/*.dmg`

## 多平台 CI 打包

推荐使用 GitHub Actions 分别在 Windows 和 macOS runner 上打包。V1 已提供 `.github/workflows/build-release.yml`。

触发方式：

1. 推送 tag，例如 `v1.0.0`
2. 或在 GitHub Actions 页面手动运行 `Build Release`

产物会上传到 workflow artifacts。
