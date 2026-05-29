# Codex Manager V1 接口文档

本文档描述 Codex Manager V1 的前后端接口、远端 NewAPI 调用、配置文件位置和 Codex 启动增强行为。

## 通用返回结构

所有 Tauri IPC 命令都返回扁平化 JSON：

```ts
type CommandResult<T> = T & {
  status: "ok" | "failed" | string;
  message: string;
};
```

前端应先检查 `status === "ok"`，失败时展示 `message`。

## 数据模型

```ts
type Account = {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  enabled: boolean;
};

type AuthUser = {
  id: number;
  username: string;
  displayName: string;
  group: string;
  role: number;
  status: number;
};

type AuthState = {
  loginMode: "newApi" | string;
  baseUrl: string;
  user: AuthUser | null;
  cookies: StoredCookie[];
  updatedAtMs: number;
};

type AppSettings = {
  codexAppPath: string;
  activeAccountId: string;
  launchExtraArgs: string[];
  pluginEnabled: boolean;
  auth: AuthState;
  accounts: Account[];
};
```

`pluginEnabled` 在 UI 中显示为“解锁插件”。开启后，启动 Codex 时会启用完整增强模式的插件入口解锁能力。

## Tauri IPC

### backend_version

获取后端版本号。

请求：

```ts
invoke("backend_version")
```

响应：

```ts
CommandResult<{ version: string }>
```

### load_settings

加载管理台配置。

请求：

```ts
invoke("load_settings")
```

响应：

```ts
CommandResult<{
  settings: AppSettings;
  settingsPath: string;
}>
```

### save_settings

保存管理台配置。

请求：

```ts
invoke("save_settings", { settings: AppSettings })
```

响应：

```ts
CommandResult<{
  settings: AppSettings;
  settingsPath: string;
}>
```

### login_user

使用 NewAPI 登录。当前 V1 只支持 `newApi`。

请求：

```ts
invoke("login_user", {
  credentials: {
    loginMode: "newApi",
    baseUrl: "https://yiciyuan.one",
    username: "user",
    password: "password"
  }
})
```

响应：

```ts
CommandResult<{ auth: AuthState }>
```

### logout_user

清除本地登录态。

请求：

```ts
invoke("logout_user")
```

响应：

```ts
CommandResult<{
  settings: AppSettings;
  settingsPath: string;
}>
```

### search_remote_keys

查询远程 KEY。必须先登录。

请求：

```ts
invoke("search_remote_keys", {
  request: { keyword: "optional keyword" }
})
```

响应：

```ts
CommandResult<{
  keyword: string;
  items: Array<{
    id: string;
    name: string;
    apiKey: string;
    group: string;
    status: unknown;
    raw: unknown;
  }>;
}>
```

### decrypt_remote_key

解密远程 KEY。必须先登录。

请求：

```ts
invoke("decrypt_remote_key", {
  request: { tokenId: "token-id" }
})
```

也兼容 `{ id: "token-id" }`。

响应：

```ts
CommandResult<{
  tokenId: string;
  apiKey: string;
}>
```

### upsert_account

新增或更新本地账号，可同时激活。

请求：

```ts
invoke("upsert_account", {
  request: {
    account: Account,
    activate: true
  }
})
```

响应：

```ts
CommandResult<{
  settings: AppSettings;
  settingsPath: string;
}>
```

### delete_account

删除本地账号。

请求：

```ts
invoke("delete_account", { id: "account-id" })
```

响应同 `load_settings`。

### activate_account

激活本地账号，并写入 Codex 配置。

请求：

```ts
invoke("activate_account", { id: "account-id" })
```

响应：

```ts
CommandResult<CodexApplyResult>
```

### apply_active_account

将当前激活账号重新写入 Codex 配置。

请求：

```ts
invoke("apply_active_account")
```

响应同 `activate_account`。

### clear_api_mode

清除 Codex Manager 写入的 API 模式配置。

请求：

```ts
invoke("clear_api_mode")
```

响应同 `activate_account`。

### read_codex_config

读取当前 Codex 配置文件内容。

请求：

```ts
invoke("read_codex_config")
```

响应：

```ts
CommandResult<{
  configPath: string;
  authPath: string;
  configToml: string;
  authJson: string;
  backupAvailable: boolean;
}>
```

### read_restore_preview

读取最近一次备份内容，用于恢复前预览。

请求：

```ts
invoke("read_restore_preview")
```

响应：

```ts
CommandResult<{
  backupPath: string;
  reason: string;
  createdAtMs: number;
  sourceAccountName: string;
  configPath: string;
  authPath: string;
  configToml: string;
  authJson: string;
}>
```

### restore_backup

恢复最近一次 Codex 配置备份。

请求：

```ts
invoke("restore_backup")
```

响应同 `activate_account`。

### open_codex_install_page

打开 Codex 下载页。Windows 打开 `/latest/win`，macOS 根据 CPU 架构打开 `/latest/mac-arm64` 或 `/latest/mac-intel`。

请求：

```ts
invoke("open_codex_install_page")
```

响应：

```ts
CommandResult<Record<string, never>>
```

### detect_codex_path

检测当前电脑上的 Codex 安装位置。V1.0.4 起参考 CodexPlusPlus 的路径解析路线：

- 优先使用用户保存的 `codexAppPath`
- 其次使用 `CODEX_APP_PATH`
- Windows 优先通过原生 `PackageManager` 查询 `OpenAI.Codex` 的 AppX/MSIX 安装位置
- Windows 再通过原生 AppModel Runtime API 按包族名解析安装目录
- Windows 再读取当前用户 AppModel Repository 注册表中的 `PackageRootFolder`
- Windows 再扫描 `C:\Program Files\WindowsApps\OpenAI.Codex_*` 并选择最高版本
- Windows 再回退到常见本地安装目录
- macOS 扫描 `/Applications` 和 `~/Applications`

请求：

```ts
invoke("detect_codex_path")
```

响应：

```ts
CommandResult<{
  appDir: string;
  executablePath: string;
  version: string;
  source: string;
  appUserModelId: string;
}>
```

### launch_codex

启动 Codex。启动前会应用当前激活账号。若 `pluginEnabled` 为 `true`，会动态选择空闲 CDP 端口并注入插件入口解锁脚本。

请求：

```ts
invoke("launch_codex", {
  request: {
    codexAppPath: "",
    extraArgs: []
  }
})
```

响应：

```ts
CommandResult<Record<string, never>>
```

### restart_codex

关闭现有 Codex 进程后重新启动，行为同 `launch_codex`。

请求：

```ts
invoke("restart_codex", {
  request: {
    codexAppPath: "",
    extraArgs: []
  }
})
```

响应：

```ts
CommandResult<Record<string, never>>
```

## Codex 配置写入

`CodexApplyResult`：

```ts
type CodexApplyResult = {
  configPath: string;
  authPath: string;
  backupPath: string;
  accountId: string;
  accountName: string;
  baseUrl: string;
};
```

写入目标：

- `CODEX_HOME/config.toml`，未设置 `CODEX_HOME` 时为 `~/.codex/config.toml`
- `CODEX_HOME/auth.json`，未设置 `CODEX_HOME` 时为 `~/.codex/auth.json`

每次应用账号、清除 API 模式前都会创建备份。

## NewAPI 远端接口

Codex Manager V1 使用兼容 NewAPI 的 HTTP 接口：

- `GET /sign-in`：预热 Cookie
- `POST /api/user/login?turnstile=`：账号密码登录
- `GET /api/user/self`：校验登录态并读取用户信息
- `GET /api/token/search?keyword={keyword}`：查询远程 KEY
- `POST /api/token/{tokenId}/key`：解密远程 KEY

请求会携带浏览器风格 `User-Agent`、`Referer`、`Origin`、Cookie，以及 `New-Api-User: {userId}`。

## 本地文件

- 管理台配置：平台配置目录下的 `CodexManager/config.json`
- 备份目录：平台数据目录下的 `CodexManager/backups`
- Codex 配置：`~/.codex/config.toml`
- Codex 认证：`~/.codex/auth.json`

## 插件解锁能力

该能力复刻 CodexPlusPlus 的完整增强模式中“插件选项解锁”和“特殊插件强制安装”的核心实现：

- 不修改 Codex 安装目录
- 不 patch `app.asar`
- 启动时添加 `--remote-debugging-port` 和 `--remote-allow-origins`
- 通过 CDP 注入渲染端脚本
- 解锁导航里的插件入口
- 解除插件安装按钮的禁用状态

为避免重启空白页，V1 会为每次启动动态选择空闲端口，并过滤用户额外参数中冲突的 CDP 参数。
