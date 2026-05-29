import { invoke } from "@tauri-apps/api/core";
import {
  FileText,
  KeyRound,
  LogOut,
  Play,
  RefreshCw,
  RotateCcw,
  Search,
  Settings,
  ShoppingBag,
  UserRound,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

type Status = "ok" | "failed" | string;

type CommandResult<T> = T & {
  status: Status;
  message: string;
};

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
  cookies: Array<Record<string, unknown>>;
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

type SettingsResult = CommandResult<{
  settings: AppSettings;
  settingsPath: string;
}>;

type LoginResult = CommandResult<{
  auth: AuthState;
}>;

type RemoteToken = {
  id: string;
  name: string;
  apiKey: string;
  group: string;
  status: unknown;
  raw: unknown;
};

type RemoteKeySearchResult = CommandResult<{
  keyword: string;
  items: RemoteToken[];
}>;

type RemoteKeyDecryptResult = CommandResult<{
  tokenId: string;
  apiKey: string;
}>;

type ApplyResult = CommandResult<{
  configPath: string;
  authPath: string;
  backupPath: string;
  accountId: string;
  accountName: string;
  baseUrl: string;
}>;

type ConfigViewResult = CommandResult<{
  configPath: string;
  authPath: string;
  configToml: string;
  authJson: string;
  backupAvailable: boolean;
}>;

type CodexPathResult = CommandResult<{
  appDir: string;
  executablePath: string;
  version: string;
  source: string;
  appUserModelId: string;
}>;

type BackupPreviewResult = CommandResult<{
  backupPath: string;
  reason: string;
  createdAtMs: number;
  sourceAccountName: string;
  configPath: string;
  authPath: string;
  configToml: string;
  authJson: string;
}>;

type Route = "account" | "market" | "settings";
type CodexLaunchState = "idle" | "starting" | "started" | "restarting";

const defaultAuth: AuthState = {
  loginMode: "newApi",
  baseUrl: "https://yiciyuan.one",
  user: null,
  cookies: [],
  updatedAtMs: 0,
};

const emptySettings: AppSettings = {
  codexAppPath: "",
  activeAccountId: "",
  launchExtraArgs: [],
  pluginEnabled: false,
  auth: defaultAuth,
  accounts: [],
};

const blankLogin = {
  loginMode: "newApi",
  baseUrl: "https://yiciyuan.one",
  username: "",
  password: "",
};

export function App() {
  const [route, setRoute] = useState<Route>("account");
  const [settings, setSettings] = useState<AppSettings>(emptySettings);
  const [settingsPath, setSettingsPath] = useState("");
  const [loginForm, setLoginForm] = useState(blankLogin);
  const [remoteKeyword, setRemoteKeyword] = useState("");
  const [remoteKeys, setRemoteKeys] = useState<RemoteToken[]>([]);
  const [remoteKeysLoading, setRemoteKeysLoading] = useState(false);
  const [decryptingId, setDecryptingId] = useState("");
  const [notice, setNotice] = useState<{ status: Status; message: string } | null>(null);
  const [configView, setConfigView] = useState<ConfigViewResult | null>(null);
  const [codexPath, setCodexPath] = useState<CodexPathResult | null>(null);
  const [codexPathChecking, setCodexPathChecking] = useState(false);
  const [configModalOpen, setConfigModalOpen] = useState(false);
  const [restorePreview, setRestorePreview] = useState<BackupPreviewResult | null>(null);
  const [restoreModalOpen, setRestoreModalOpen] = useState(false);
  const [restoreLoading, setRestoreLoading] = useState(false);
  const [codexLaunchState, setCodexLaunchState] = useState<CodexLaunchState>("idle");

  const activeAccount = useMemo(
    () => settings.accounts.find((account) => account.id === settings.activeAccountId),
    [settings],
  );
  const user = settings.auth.user;

  useEffect(() => {
    void refresh();
    void readConfig();
    void detectCodexPath();
  }, []);

  useEffect(() => {
    if (user) void searchRemoteKeys(true);
  }, [user?.id]);

  const call = <T,>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args);

  async function refresh() {
    const result = await run(() => call<SettingsResult>("load_settings"));
    if (!result) return;
    setSettings(result.settings);
    setSettingsPath(result.settingsPath);
    setLoginForm((current) => ({
      ...current,
      baseUrl: result.settings.auth.baseUrl || current.baseUrl,
      loginMode: result.settings.auth.loginMode || "newApi",
    }));
    await detectCodexPath();
  }

  async function login() {
    const result = await run(() => call<LoginResult>("login_user", { credentials: loginForm }));
    if (!result) return;
    setSettings((current) => ({ ...current, auth: result.auth }));
    setLoginForm((current) => ({ ...current, password: "" }));
    show(result);
    await searchRemoteKeys(true);
  }

  async function logout() {
    const result = await run(() => call<SettingsResult>("logout_user"));
    if (!result) return;
    setSettings(result.settings);
    setRemoteKeys([]);
    show(result);
  }

  async function searchRemoteKeys(silent = false) {
    setRemoteKeysLoading(true);
    try {
      const result = await run(() =>
        call<RemoteKeySearchResult>("search_remote_keys", {
          request: { keyword: remoteKeyword },
        }),
      );
      if (!result) return;
      setRemoteKeys(result.items);
      if (!silent) show(result);
    } finally {
      setRemoteKeysLoading(false);
    }
  }

  async function useRemoteKey(item: RemoteToken) {
    if (!item.id) return;
    setDecryptingId(item.id);
    const result = await run(() =>
      call<RemoteKeyDecryptResult>("decrypt_remote_key", {
        request: { tokenId: item.id },
      }),
    );
    setDecryptingId("");
    if (!result) return;

    const account: Account = {
      id: `remote-${item.id}`,
      name: item.name || `远程 KEY ${item.id}`,
      baseUrl: apiBaseUrlForAuth(settings.auth.baseUrl),
      apiKey: result.apiKey,
      enabled: true,
    };
    const saved = await run(() =>
      call<SettingsResult>("upsert_account", {
        request: { account, activate: true },
      }),
    );
    if (!saved) return;
    setSettings(saved.settings);
    await activateAccount(account.id, true);
  }

  async function activateAccount(id: string, silent = false) {
    const result = await run(() => call<ApplyResult>("activate_account", { id }));
    if (!result) return;
    await refresh();
    await readConfig();
    if (!silent) show(result);
    else show({ status: result.status, message: `已切换到 ${result.accountName}` });
  }

  async function openRestorePreview() {
    const result = await run(() => call<BackupPreviewResult>("read_restore_preview"));
    if (!result) return;
    setRestorePreview(result);
    setRestoreModalOpen(true);
  }

  async function confirmRestoreBackup() {
    setRestoreLoading(true);
    try {
      const result = await run(() => call<ApplyResult>("restore_backup"));
      if (!result) return;
      await readConfig();
      setRestoreModalOpen(false);
      show(result);
    } finally {
      setRestoreLoading(false);
    }
  }

  async function saveGlobalSettings() {
    const result = await run(() => call<SettingsResult>("save_settings", { settings }));
    if (!result) return;
    setSettings(result.settings);
    setSettingsPath(result.settingsPath);
    show(result);
  }

  async function togglePlugin() {
    if (codexLaunchState === "starting" || codexLaunchState === "restarting") return;
    const next = { ...settings, pluginEnabled: !settings.pluginEnabled };
    setSettings(next);
    if (next.pluginEnabled) {
      setNotice({ status: "ok", message: "解锁插件会重启 Codex，正在保存设置..." });
    }
    const result = await run(() => call<SettingsResult>("save_settings", { settings: next }));
    if (!result) return;
    setSettings(result.settings);
    if (result.settings.pluginEnabled) {
      setNotice({ status: "ok", message: "解锁插件会重启 Codex，正在重启..." });
      await restartCodex(result.settings, "插件解锁已启用，Codex 已重启");
      return;
    }
    show({
      status: result.status,
      message: "插件解锁已停用",
    });
  }

  async function installCodex() {
    const result = await run(() => call<CommandResult<Record<string, never>>>("open_codex_install_page"));
    if (result) show(result);
  }

  async function detectCodexPath() {
    setCodexPathChecking(true);
    setNotice({ status: "ok", message: "正在检测 Codex 安装位置..." });
    try {
      const result = await run(() => call<CodexPathResult>("detect_codex_path"));
      if (result) {
        setCodexPath(result);
        show(result);
      }
    } finally {
      setCodexPathChecking(false);
    }
  }

  async function openConfigModal() {
    await readConfig();
    setConfigModalOpen(true);
  }

  async function readConfig() {
    const result = await run(() => call<ConfigViewResult>("read_codex_config"));
    if (result) setConfigView(result);
  }

  async function launchCodex() {
    if (codexLaunchState === "starting" || codexLaunchState === "started") return;
    setCodexLaunchState("starting");
    const result = await run(() =>
      call<CommandResult<Record<string, never>>>("launch_codex", {
        request: {
          codexAppPath: settings.codexAppPath,
          extraArgs: [],
        },
      }),
    );
    if (result?.status === "ok") {
      await detectCodexPath();
      setCodexLaunchState("started");
      show(result);
      return;
    }
    setCodexLaunchState("idle");
    if (result) show(result);
  }

  async function restartCodex(settingsOverride = settings, successMessage?: string) {
    if (codexLaunchState === "starting" || codexLaunchState === "restarting") return;
    const launchSettings = settingsOverride;
    setCodexLaunchState("restarting");
    const result = await run(() =>
      call<CommandResult<Record<string, never>>>("restart_codex", {
        request: {
          codexAppPath: launchSettings.codexAppPath,
          extraArgs: [],
        },
      }),
    );
    if (result?.status === "ok") {
      await detectCodexPath();
      setCodexLaunchState("started");
      show({ status: result.status, message: successMessage || result.message });
      return;
    }
    setCodexLaunchState("idle");
    if (result) show(result);
  }

  async function run<T>(task: () => Promise<T>): Promise<T | null> {
    try {
      return await task();
    } catch (error) {
      setNotice({ status: "failed", message: stringifyError(error) });
      return null;
    }
  }

  function show(result: { status: Status; message: string }) {
    setNotice({ status: result.status, message: result.message });
  }

  return (
    <main className="appShell">
      <aside className="side">
        <div className="logoMark">
          <KeyRound size={22} />
        </div>
        <nav className="nav">
          <button className={route === "account" ? "isActive" : ""} onClick={() => setRoute("account")} type="button">
            <UserRound size={16} /> 账户
          </button>
          <button className={route === "market" ? "isActive" : ""} onClick={() => setRoute("market")} type="button">
            <ShoppingBag size={16} /> 商店
          </button>
          <button className={route === "settings" ? "isActive" : ""} onClick={() => setRoute("settings")} type="button">
            <Settings size={16} /> 设置
          </button>
        </nav>
        <div className="sideBottom">
          <button className="blackButton" onClick={() => restartCodex()} type="button" disabled={codexLaunchState === "restarting"}>
            {codexLaunchState === "restarting" ? "重启中" : "重启"}
          </button>
          <button className="blackButton" onClick={() => setRoute("account")} type="button">
            {user?.displayName || user?.username || "未登录"}
          </button>
        </div>
      </aside>

      <section className="content">
        {notice ? <div className={`notice ${notice.status === "ok" ? "ok" : "failed"}`}>{notice.message}</div> : null}
        {route === "account" ? accountView() : null}
        {route === "market" ? marketView() : null}
        {route === "settings" ? settingsView() : null}
        {configModalOpen ? configModal() : null}
        {restoreModalOpen ? restoreModal() : null}
      </section>
    </main>
  );

  function accountView() {
    return (
      <>
        <h1>用户中心</h1>
        {user ? loggedInCard(user) : loginCard()}
        {user ? keyManager() : null}
      </>
    );
  }

  function loginCard() {
    return (
      <section className="card loginCard">
        <div className="sectionHead">
          <div>
            <h2>登录</h2>
            <p>选择 newApi 登录方式后，可查询和使用远程 KEY。</p>
          </div>
        </div>
        <div className="formGrid">
          <label>
            登录方式
            <select
              value={loginForm.loginMode}
              onChange={(event) => setLoginForm({ ...loginForm, loginMode: event.target.value })}
            >
              <option value="newApi">newApi</option>
            </select>
          </label>
          <label>
            服务地址
            <input
              value={loginForm.baseUrl}
              onChange={(event) => setLoginForm({ ...loginForm, baseUrl: event.target.value })}
            />
          </label>
          <label>
            账号
            <input
              autoComplete="username"
              value={loginForm.username}
              onChange={(event) => setLoginForm({ ...loginForm, username: event.target.value })}
            />
          </label>
          <label>
            密码
            <input
              autoComplete="current-password"
              type="password"
              value={loginForm.password}
              onChange={(event) => setLoginForm({ ...loginForm, password: event.target.value })}
            />
          </label>
        </div>
        <button className="primaryButton" onClick={login} type="button">
          登录
        </button>
      </section>
    );
  }

  function loggedInCard(currentUser: AuthUser) {
    return (
      <section className="profileCard">
        <div className="avatar">
          <KeyRound size={34} />
        </div>
        <div className="profileText">
          <h2>{currentUser.displayName || currentUser.username}</h2>
          <p>用户名：{currentUser.username}</p>
          <p>用户组：{currentUser.group || "-"}</p>
        </div>
        <button onClick={logout} type="button">
          <LogOut size={16} /> 退出登录
        </button>
      </section>
    );
  }

  function keyManager() {
    return (
      <section className="keySection">
        <div className="sectionHead">
          <div>
            <h1>KEY 管理</h1>
          </div>
        </div>
        <div className="searchRow">
          <input
            placeholder="搜索"
            value={remoteKeyword}
            onChange={(event) => setRemoteKeyword(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !remoteKeysLoading) void searchRemoteKeys();
            }}
          />
          <button className="primaryButton" onClick={() => searchRemoteKeys()} type="button" disabled={remoteKeysLoading}>
            <Search size={16} /> {remoteKeysLoading ? "查询中" : "查询"}
          </button>
        </div>
        <div className="remoteList">
          {remoteKeysLoading ? <div className="empty loadingBox">正在查询 KEY，请稍候...</div> : null}
          {!remoteKeysLoading && remoteKeys.length === 0 ? <div className="empty">暂无远端 KEY</div> : null}
          {remoteKeys.map((item) => {
            const active = settings.activeAccountId === `remote-${item.id}`;
            return (
              <article className={`remoteItem ${active ? "active" : ""}`} key={item.id || item.name}>
                <div className="miniIcon">
                  <KeyRound size={20} />
                </div>
                <div className="remoteTitle">
                  <strong>{item.name || "未命名 KEY"}</strong>
                  <span>
                    {item.group || "-"} <em>{statusText(item.status)}</em>
                  </span>
                </div>
                <code>{maskKey(item.apiKey)}</code>
                <button onClick={() => useRemoteKey(item)} type="button" disabled={!item.id || decryptingId === item.id || active}>
                  {decryptingId === item.id ? "解密中" : "解密"}
                </button>
                <button className={active ? "useState active" : "useState"} onClick={() => useRemoteKey(item)} type="button" disabled={active}>
                  <span />
                  {active ? "使用中" : "使用"}
                </button>
              </article>
            );
          })}
        </div>
      </section>
    );
  }

  function marketView() {
    return (
      <>
        <h1>商店</h1>
        <section className="card placeholder">脚本市场和扩展推荐入口预留。</section>
      </>
    );
  }

  function settingsView() {
    return (
      <>
        <section className="settingsPanel">
          <header className="settingsTitle">
            <h1>设置</h1>
            <span />
          </header>

          <div className="settingRow">
            <h2>启动 Codex</h2>
            <div className="settingActions">
              <button
                className={codexLaunchState === "started" ? "successButton" : "primaryButton"}
                onClick={launchCodex}
                type="button"
                disabled={codexLaunchState === "starting" || codexLaunchState === "started"}
              >
                {codexLaunchState === "starting" ? "启动中..." : codexLaunchState === "started" ? "已启动" : "启动"}
              </button>
              <button onClick={() => restartCodex()} type="button" disabled={codexLaunchState === "starting" || codexLaunchState === "restarting"}>
                {codexLaunchState === "restarting" ? "重启中..." : "重启"}
              </button>
            </div>
          </div>

          <div className="settingRow">
            <h2>安装 Codex</h2>
            <button onClick={installCodex} type="button">
              安装
            </button>
          </div>

          <div className="settingRow pathRow">
            <h2>Codex 位置</h2>
            <div className="pathInfo">
              <code>
                {codexPathChecking
                  ? "正在检测 Codex 安装位置..."
                  : codexPath?.executablePath || codexPath?.message || "未检测到 Codex 安装位置"}
              </code>
              {codexPath?.status === "ok" ? (
                <span>
                  {codexPath.source || "auto"}
                  {codexPath.version ? ` · ${codexPath.version}` : ""}
                </span>
              ) : null}
              <button onClick={detectCodexPath} type="button" disabled={codexPathChecking}>
                <RefreshCw size={14} /> {codexPathChecking ? "检测中..." : "重新检测"}
              </button>
            </div>
          </div>

          <div className="settingRow">
            <h2>解锁插件</h2>
            <button
              aria-label="解锁插件"
              className={`switch ${settings.pluginEnabled ? "isOn" : ""}`}
              disabled={codexLaunchState === "starting" || codexLaunchState === "restarting"}
              onClick={togglePlugin}
              type="button"
            >
              <span />
            </button>
          </div>

          <div className="settingRow">
            <h2>恢复配置</h2>
            <button onClick={openRestorePreview} type="button" disabled={!configView?.backupAvailable}>
              恢复
            </button>
          </div>

          <div className="settingRow">
            <h2>查看配置</h2>
            <button onClick={openConfigModal} type="button">
              查看
            </button>
          </div>
        </section>
      </>
    );
  }

  function configModal() {
    return (
      <div className="modalBackdrop" onClick={() => setConfigModalOpen(false)}>
        <div className="configModal" onClick={(event) => event.stopPropagation()}>
          <div className="modalHead">
            <div>
              <h2>当前 Codex 配置</h2>
              <p>{settingsPath || "配置文件尚未创建"}</p>
            </div>
            <button onClick={() => setConfigModalOpen(false)} type="button">
              关闭
            </button>
          </div>
          <div className="previewGrid">
            <Preview title="config.toml" path={configView?.configPath} text={configView?.configToml} />
            <Preview title="auth.json" path={configView?.authPath} text={configView?.authJson} />
          </div>
        </div>
      </div>
    );
  }

  function restoreModal() {
    return (
      <div className="modalBackdrop" onClick={() => setRestoreModalOpen(false)}>
        <div className="configModal" onClick={(event) => event.stopPropagation()}>
          <div className="modalHead">
            <div>
              <h2>恢复配置预览</h2>
              <p>{restorePreview?.backupPath || "-"}</p>
            </div>
            <button onClick={() => setRestoreModalOpen(false)} type="button" disabled={restoreLoading}>
              关闭
            </button>
          </div>
          <div className="restoreMeta">
            <span>备份时间：{formatTime(restorePreview?.createdAtMs)}</span>
            <span>备份来源：{restorePreview?.reason || "-"}</span>
            <span>账号：{restorePreview?.sourceAccountName || "-"}</span>
          </div>
          <div className="previewGrid">
            <Preview title="config.toml" path={restorePreview?.configPath} text={restorePreview?.configToml} />
            <Preview title="auth.json" path={restorePreview?.authPath} text={restorePreview?.authJson} />
          </div>
          <div className="modalActions">
            <button onClick={() => setRestoreModalOpen(false)} type="button" disabled={restoreLoading}>
              取消
            </button>
            <button className="primaryButton" onClick={confirmRestoreBackup} type="button" disabled={restoreLoading}>
              {restoreLoading ? "恢复中..." : "确认恢复"}
            </button>
          </div>
        </div>
      </div>
    );
  }
}

function Preview({ title, path, text }: { title: string; path?: string; text?: string }) {
  return (
    <div className="preview">
      <div>
        <strong>{title}</strong>
        <span>{path || "-"}</span>
      </div>
      <pre>{text || ""}</pre>
    </div>
  );
}

function apiBaseUrlForAuth(baseUrl: string) {
  const clean = (baseUrl || "https://yiciyuan.one").replace(/\/+$/, "");
  return clean.endsWith("/v1") ? clean : `${clean}/v1`;
}

function maskKey(value: string) {
  if (!value) return "-";
  if (value.length <= 10) return "*".repeat(value.length);
  return `${value.slice(0, 5)}**********${value.slice(-4)}`;
}

function statusText(value: unknown) {
  if (value === null || value === undefined || value === "") return "-";
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}

function formatTime(value?: number) {
  if (!value) return "-";
  return new Date(value).toLocaleString();
}

function stringifyError(error: unknown) {
  if (error instanceof Error) return error.message;
  return String(error);
}
