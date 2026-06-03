import { invoke } from "@tauri-apps/api/core";
import {
  ChevronDown,
  Download,
  FileText,
  KeyRound,
  LogOut,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  Search,
  Settings,
  ShoppingBag,
  Trash2,
  UserRound,
  X,
} from "lucide-react";
import type { JSX } from "react";
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

type LoginAccount = {
  id: string;
  name: string;
  auth: AuthState;
};

type AppSettings = {
  codexAppPath: string;
  activeAccountId: string;
  launchExtraArgs: string[];
  pluginEnabled: boolean;
  auth: AuthState;
  activeLoginId: string;
  loginAccounts: LoginAccount[];
  updateManifestUrl: string;
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

type BackendVersionResult = CommandResult<{
  version: string;
}>;

type UpdateResult = CommandResult<{
  currentVersion: string;
  latestVersion: string | null;
  releaseSummary: string;
  assetName: string | null;
  assetUrl: string | null;
  updateAvailable: boolean;
}>;

type InstallUpdateResult = CommandResult<{
  assetName: string;
  assetUrl: string;
  installerPath: string;
}>;

type ProviderSyncResult = CommandResult<{
  syncStatus: "skipped" | "synced" | string;
  targetProvider: string;
  backupDir: string | null;
  changedSessionFiles: number;
  skippedLockedRolloutFiles: string[];
  sqliteRowsUpdated: number;
  sqliteProviderRowsUpdated: number;
  sqliteUserEventRowsUpdated: number;
  sqliteCwdRowsUpdated: number;
  updatedWorkspaceRoots: number;
  encryptedContentWarning: string | null;
  syncMessage: string;
}>;

type CodexPreferenceResult = CommandResult<{
  configPath: string;
  backupPath: string;
  localeOverride: string;
  developerInstructions: string;
}>;

type Route = "account" | "keys" | "market" | "settings";
type CodexLaunchState = "idle" | "starting" | "started" | "restarting";

type ManualKeyForm = {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
};

const defaultAuth: AuthState = {
  loginMode: "newApi",
  baseUrl: "",
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
  activeLoginId: "",
  loginAccounts: [],
  updateManifestUrl: "",
  accounts: [],
};

const blankLogin = {
  loginMode: "newApi",
  baseUrl: "",
  username: "",
  password: "",
};

const blankManualKey: ManualKeyForm = {
  id: "",
  name: "",
  baseUrl: "https://api.openai.com/v1",
  apiKey: "",
};

const defaultChineseInstructions =
  "请始终使用简体中文回答，除非我明确要求使用其他语言。代码、命令、错误信息、配置项和专有名词保持原文。";

export function App() {
  const [route, setRoute] = useState<Route>("keys");
  const [settings, setSettings] = useState<AppSettings>(emptySettings);
  const [settingsPath, setSettingsPath] = useState("");
  const [loginForm, setLoginForm] = useState(blankLogin);
  const [expandedLoginId, setExpandedLoginId] = useState("");
  const [manualKeyForm, setManualKeyForm] = useState<ManualKeyForm>(blankManualKey);
  const [manualKeySaving, setManualKeySaving] = useState(false);
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
  const [appVersion, setAppVersion] = useState("");
  const [updateInfo, setUpdateInfo] = useState<UpdateResult | null>(null);
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updateInstalling, setUpdateInstalling] = useState(false);
  const [sessionRepairing, setSessionRepairing] = useState(false);
  const [sessionRepairResult, setSessionRepairResult] = useState<ProviderSyncResult | null>(null);
  const [codexPreferences, setCodexPreferences] = useState<CodexPreferenceResult | null>(null);
  const [codexPreferenceForm, setCodexPreferenceForm] = useState({
    localeOverride: "",
    developerInstructions: "",
  });
  const [codexPreferenceSaving, setCodexPreferenceSaving] = useState(false);

  const activeAccount = useMemo(
    () => settings.accounts.find((account) => account.id === settings.activeAccountId),
    [settings],
  );
  const activeLoginAccount = useMemo(
    () => settings.loginAccounts.find((account) => account.id === settings.activeLoginId),
    [settings],
  );
  const user = activeLoginAccount?.auth.user ?? settings.auth.user;

  useEffect(() => {
    void refresh();
    void readConfig();
    void readCodexPreferences();
    void detectCodexPath();
    void loadAppVersion();
  }, []);

  useEffect(() => {
    if (!settings.loginAccounts.length) {
      setExpandedLoginId("");
      setRemoteKeys([]);
      return;
    }
    if (expandedLoginId && !settings.loginAccounts.some((account) => account.id === expandedLoginId)) {
      setExpandedLoginId("");
      setRemoteKeys([]);
    }
  }, [settings.loginAccounts, expandedLoginId]);

  useEffect(() => {
    if (expandedLoginId) void searchRemoteKeys(true, expandedLoginId);
  }, [expandedLoginId]);

  const call = <T,>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args);

  async function refresh() {
    const result = await run(() => call<SettingsResult>("load_settings"));
    if (!result) return null;
    setSettings(result.settings);
    setSettingsPath(result.settingsPath);
    setLoginForm((current) => ({
      ...current,
      baseUrl: result.settings.auth.user ? result.settings.auth.baseUrl : current.baseUrl,
      loginMode: result.settings.auth.loginMode || "newApi",
    }));
    await detectCodexPath();
    return result.settings;
  }

  async function loadAppVersion() {
    const result = await run(() => call<BackendVersionResult>("backend_version"));
    if (!result) return;
    setAppVersion(result.version);
  }

  async function checkAppUpdate() {
    setUpdateChecking(true);
    try {
      const result = await run(() => call<UpdateResult>("check_update"));
      if (!result) return;
      setUpdateInfo(result);
      show(result);
    } finally {
      setUpdateChecking(false);
    }
  }

  async function installLatestUpdate() {
    if (!updateInfo?.assetUrl) {
      setNotice({ status: "failed", message: "没有可用的安装包下载地址" });
      return;
    }
    setUpdateInstalling(true);
    try {
      const result = await run(() =>
        call<InstallUpdateResult>("install_latest_update", {
          request: {
            assetName: updateInfo.assetName || "",
            assetUrl: updateInfo.assetUrl,
            latestVersion: updateInfo.latestVersion || "",
          },
        }),
      );
      if (!result) return;
      show(result);
    } finally {
      setUpdateInstalling(false);
    }
  }

  async function login() {
    const baseUrl = loginForm.baseUrl.trim();
    if (!baseUrl) {
      setNotice({ status: "failed", message: "请填写服务地址" });
      return;
    }
    const result = await run(() => call<LoginResult>("login_user", { credentials: { ...loginForm, baseUrl } }));
    if (!result) return;
    setLoginForm((current) => ({ ...current, password: "" }));
    const nextSettings = await refresh();
    const nextLogin = nextSettings?.loginAccounts.find(
      (account) => account.auth.baseUrl === result.auth.baseUrl && account.auth.user?.id === result.auth.user?.id,
    );
    if (nextLogin) {
      setExpandedLoginId(nextLogin.id);
      await searchRemoteKeys(true, nextLogin.id);
    }
    show(result);
  }

  async function logout() {
    const result = await run(() => call<SettingsResult>("logout_user"));
    if (!result) return;
    setSettings(result.settings);
    setRemoteKeys([]);
    show(result);
  }

  async function activateLoginAccount(loginId: string) {
    const result = await run(() => call<SettingsResult>("activate_login_account", { id: loginId }));
    if (!result) return;
    setSettings(result.settings);
    setExpandedLoginId(loginId);
    show({ status: result.status, message: "已切换登录账号" });
  }

  async function deleteLoginAccount(loginId: string) {
    const result = await run(() => call<SettingsResult>("delete_login_account", { id: loginId }));
    if (!result) return;
    setSettings(result.settings);
    if (expandedLoginId === loginId) {
      setRemoteKeys([]);
      setExpandedLoginId(result.settings.activeLoginId || result.settings.loginAccounts[0]?.id || "");
    }
    show({ status: result.status, message: "已删除登录账号" });
  }

  async function searchRemoteKeys(silent = false, loginId = expandedLoginId) {
    if (!loginId) return;
    setRemoteKeys([]);
    setRemoteKeysLoading(true);
    try {
      const result = await run(() =>
        call<RemoteKeySearchResult>("search_remote_keys", {
          request: { keyword: remoteKeyword, loginId },
        }),
      );
      if (!result) return;
      setRemoteKeys(result.items);
      if (!silent) show(result);
    } finally {
      setRemoteKeysLoading(false);
    }
  }

  async function importRemoteKey(item: RemoteToken, activate: boolean, loginAccount = activeLoginAccount) {
    if (!item.id) return;
    if (!loginAccount) {
      setNotice({ status: "failed", message: "请先选择登录账号" });
      return;
    }
    setDecryptingId(item.id);
    const result = await run(() =>
      call<RemoteKeyDecryptResult>("decrypt_remote_key", {
        request: { tokenId: item.id, loginId: loginAccount.id },
      }),
    );
    setDecryptingId("");
    if (!result) return;

    const account: Account = {
      id: remoteAccountId(item, loginAccount),
      name: item.name || `远程 KEY ${item.id}`,
      baseUrl: apiBaseUrlForAuth(loginAccount.auth.baseUrl),
      apiKey: result.apiKey,
      enabled: true,
    };
    const saved = await run(() =>
      call<SettingsResult>("upsert_account", {
        request: { account, activate },
      }),
    );
    if (!saved) return;
    setSettings(saved.settings);
    if (activate) {
      await activateAccount(account.id, true);
    } else {
      show({ status: "ok", message: `已同步 ${account.name} 到本地 KEY 管理` });
    }
  }

  async function syncAllRemoteKeys(loginAccount?: LoginAccount) {
    const targetLogin =
      loginAccount ?? settings.loginAccounts.find((account) => account.id === expandedLoginId) ?? activeLoginAccount;
    if (!targetLogin || !remoteKeys.length) return;
    let imported = 0;
    for (const item of remoteKeys) {
      if (!item.id) continue;
      const existingId = remoteAccountId(item, targetLogin);
      if (settings.accounts.some((a) => a.id === existingId)) continue;
      const decrypted = await run(() =>
        call<RemoteKeyDecryptResult>("decrypt_remote_key", {
          request: { tokenId: item.id, loginId: targetLogin.id },
        }),
      );
      if (!decrypted) continue;
      const account: Account = {
        id: existingId,
        name: item.name || `远程 KEY ${item.id}`,
        baseUrl: apiBaseUrlForAuth(targetLogin.auth.baseUrl),
        apiKey: decrypted.apiKey,
        enabled: true,
      };
      const saved = await run(() =>
        call<SettingsResult>("upsert_account", {
          request: { account, activate: false },
        }),
      );
      if (saved) {
        setSettings(saved.settings);
        imported += 1;
      }
    }
    show({
      status: "ok",
      message: imported ? `已同步 ${imported} 个远程 KEY 到本地 KEY 管理` : "没有新的远程 KEY 需要同步到本地 KEY 管理",
    });
  }

  async function saveManualKey() {
    const name = manualKeyForm.name.trim() || "免登录 KEY";
    const baseUrl = manualKeyForm.baseUrl.trim();
    let apiKey = manualKeyForm.apiKey.trim();

    // editing an existing entry: keep the saved api_key if user left the field blank
    const existing = manualKeyForm.id
      ? settings.accounts.find((a) => a.id === manualKeyForm.id)
      : undefined;
    if (!apiKey && existing?.apiKey) {
      apiKey = existing.apiKey;
    }

    if (!baseUrl || !apiKey) {
      setNotice({ status: "failed", message: "请填写 Base URL 和 API KEY" });
      return;
    }
    const account: Account = {
      id: manualKeyForm.id || `manual-${globalThis.crypto?.randomUUID?.() || Date.now()}`,
      name,
      baseUrl,
      apiKey,
      enabled: true,
    };
    setManualKeySaving(true);
    try {
      const saved = await run(() =>
        call<SettingsResult>("upsert_account", {
          request: { account, activate: true },
        }),
      );
      if (!saved) return;
      setSettings(saved.settings);
      await activateAccount(account.id, true);
      // belt-and-suspenders: ensure Codex config.toml & auth.json are rewritten
      // from the freshly saved account, even if disk state was stale moments ago.
      await run(() => call<ApplyResult>("apply_active_account"));
      await readConfig();
      setManualKeyForm(blankManualKey);
    } finally {
      setManualKeySaving(false);
    }
  }

  function editSavedAccount(account: Account) {
    setManualKeyForm({
      id: account.id,
      name: account.name,
      baseUrl: account.baseUrl,
      apiKey: "",
    });
  }

  function cancelEdit() {
    setManualKeyForm(blankManualKey);
  }

  async function switchSavedAccount(account: Account) {
    await activateAccount(account.id, true);
    await run(() => call<ApplyResult>("apply_active_account"));
    await readConfig();
  }

  async function deleteSavedAccount(account: Account) {
    const result = await run(() => call<SettingsResult>("delete_account", { id: account.id }));
    if (!result) return;
    setSettings(result.settings);
    show({ status: result.status, message: `已删除 ${account.name}` });
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

  async function repairHistoricalSessions() {
    if (sessionRepairing) return;
    setSessionRepairing(true);
    setNotice({ status: "ok", message: "正在修复历史会话..." });
    try {
      const result = await run(() => call<ProviderSyncResult>("repair_historical_sessions"));
      if (!result) return;
      setSessionRepairResult(result);
      show(result);
    } finally {
      setSessionRepairing(false);
    }
  }

  async function readCodexPreferences() {
    const result = await run(() => call<CodexPreferenceResult>("read_codex_preferences"));
    if (!result) return;
    setCodexPreferences(result);
    setCodexPreferenceForm({
      localeOverride: result.localeOverride || "",
      developerInstructions: result.developerInstructions || "",
    });
  }

  async function saveCodexPreferences() {
    setCodexPreferenceSaving(true);
    setNotice({ status: "ok", message: "正在保存设置，随后重启 Codex..." });
    try {
      const result = await run(() =>
        call<CodexPreferenceResult>("save_codex_preferences", {
          preferences: codexPreferenceForm,
        }),
      );
      if (!result) return;
      setCodexPreferences(result);
      setCodexPreferenceForm({
        localeOverride: result.localeOverride || "",
        developerInstructions: result.developerInstructions || "",
      });
      await readConfig();
      await restartCodex(settings, "设置已保存，Codex 已重启");
    } finally {
      setCodexPreferenceSaving(false);
    }
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
    setNotice({ status: result.status || "ok", message: result.message || "操作完成" });
  }

  function toggleLoginPanel(loginId: string) {
    if (expandedLoginId === loginId) {
      setExpandedLoginId("");
      setRemoteKeys([]);
      return;
    }
    setExpandedLoginId(loginId);
  }

  const navItems: Array<{ id: Route; label: string; icon: JSX.Element }> = [
    { id: "keys", label: "本地模式", icon: <KeyRound size={17} /> },
    { id: "account", label: "登录配置", icon: <UserRound size={17} /> },
    { id: "market", label: "技能商店", icon: <ShoppingBag size={17} /> },
    { id: "settings", label: "设置", icon: <Settings size={17} /> },
  ];
  const restarting = codexLaunchState === "restarting";

  return (
    <main className="appShell">
      <aside className="side">
        <div className="brand">
          <div className="logoMark">
            <KeyRound size={20} />
          </div>
          <span className="brandName">codex助手</span>
        </div>
        <nav className="nav">
          {navItems.map((item) => (
            <button
              key={item.id}
              className={`navItem ${route === item.id ? "isActive" : ""}`}
              onClick={() => setRoute(item.id)}
              type="button"
            >
              <span className="navBar" />
              {item.icon}
              <span>{item.label}</span>
            </button>
          ))}
        </nav>
        <div className="sideBottom">
          <button
            className="identityChip"
            onClick={() => setRoute("account")}
            type="button"
            title={user ? "查看登录配置" : "前往登录配置"}
          >
            <span className={`statusDot ${user ? "online" : ""}`} />
            <span className="identityName">{user?.displayName || user?.username || "未登录"}</span>
          </button>
          <button
            className="ghostIconButton"
            onClick={() => restartCodex()}
            type="button"
            disabled={restarting}
            title="重启 Codex"
          >
            <RotateCcw size={15} className={restarting ? "spin" : ""} />
            {restarting ? "重启中" : "重启 Codex"}
          </button>
        </div>
      </aside>

      <section className="content">
        {notice ? (
          <div className={`notice ${notice.status === "ok" ? "ok" : "failed"}`} role="status">
            {notice.message}
          </div>
        ) : null}
        {route === "keys" ? keysView() : null}
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
      <div className="accountPage">
        <header className="pageHead">
          <h1>登录配置</h1>
          <p>登录不同平台后会生成账号面板，可展开查询并同步对应平台的 KEY。</p>
        </header>
        {loginCard()}
        {loginAccountsCard()}
      </div>
    );
  }

  function keysView() {
    return (
      <>
        <header className="pageHead">
          <h1>本地模式 KEY 配置</h1>
          {activeAccount ? <span className="activeAccountBadge">当前：{activeAccount.name}</span> : null}
        </header>
        {manualKeyCard()}
        {savedKeysCard()}
      </>
    );
  }

  function manualKeyCard() {
    const editing = !!manualKeyForm.id;
    return (
      <section className="card manualKeyCard">
        <div className="sectionHead">
          <div>
            <h2>{editing ? "编辑 KEY" : "新增 KEY"}</h2>
          </div>
          {editing ? (
            <button className="ghostButton" onClick={cancelEdit} type="button">
              <X size={14} /> 取消编辑
            </button>
          ) : null}
        </div>
        <div className="formGrid">
          <label>
            名称
            <input
              placeholder="例如：个人 KEY"
              value={manualKeyForm.name}
              onChange={(event) => setManualKeyForm({ ...manualKeyForm, name: event.target.value })}
            />
          </label>
          <label>
            Base URL
            <input
              placeholder="https://api.openai.com/v1"
              value={manualKeyForm.baseUrl}
              onChange={(event) => setManualKeyForm({ ...manualKeyForm, baseUrl: event.target.value })}
            />
          </label>
          <label className="wideField">
            API KEY
            <input
              autoComplete="off"
              placeholder={editing ? "留空保持原 KEY 不变" : "sk-..."}
              type="password"
              value={manualKeyForm.apiKey}
              onChange={(event) => setManualKeyForm({ ...manualKeyForm, apiKey: event.target.value })}
            />
          </label>
        </div>
        <button className="primaryButton" onClick={saveManualKey} type="button" disabled={manualKeySaving}>
          <Plus size={15} /> {manualKeySaving ? "保存中" : editing ? "保存修改并切换" : "保存并切换"}
        </button>
      </section>
    );
  }

  function savedKeysCard() {
    if (!settings.accounts.length) {
      return (
        <section className="card">
          <div className="empty">尚未保存 KEY，使用上方表单添加，或在「登录配置」页登录后同步远程 KEY。</div>
        </section>
      );
    }
    return (
      <section className="card">
        <div className="sectionHead">
          <div>
            <h2>已保存的 KEY</h2>
          </div>
          {user ? (
            <button className="ghostButton" onClick={() => syncAllRemoteKeys()} type="button">
              <RefreshCw size={14} /> 同步到本地 KEY 管理
            </button>
          ) : null}
        </div>
        <div className="accountList">
          {settings.accounts.map((account) => {
            const active = settings.activeAccountId === account.id;
            const isRemote = account.id.startsWith("remote-");
            return (
              <article className={`accountItem ${active ? "active" : ""}`} key={account.id}>
                <div className="miniIcon">
                  <KeyRound size={16} />
                </div>
                <div className="accountTitle">
                  <strong>{account.name}</strong>
                  <span>
                    {isRemote ? "远程" : "本地"} · {account.baseUrl}
                  </span>
                </div>
                <code>{maskKey(account.apiKey)}</code>
                <button
                  className={`useButton ${active ? "active" : ""}`}
                  onClick={() => switchSavedAccount(account)}
                  type="button"
                  disabled={active}
                >
                  <span className="useDot" />
                  {active ? "使用中" : "切换"}
                </button>
                {!isRemote ? (
                  <button className="iconButton" onClick={() => editSavedAccount(account)} type="button" title="编辑">
                    <Pencil size={14} />
                  </button>
                ) : null}
                <button className="iconButton danger" onClick={() => deleteSavedAccount(account)} type="button" title="删除">
                  <Trash2 size={15} />
                </button>
              </article>
            );
          })}
        </div>
      </section>
    );
  }

  function loginCard() {
    return (
      <section className="card loginCard">
        <div className="sectionHead">
          <div>
            <h2>登录</h2>
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
              placeholder="https://中转地址"
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

  function loginAccountsCard() {
    if (!settings.loginAccounts.length) {
      return (
        <section className="card">
          <div className="empty">还没有登录账号，登录 newApi 平台后会在这里生成账号面板。</div>
        </section>
      );
    }

    return (
      <section className="loginAccountList">
        <div className="sectionHead slimHead">
          <div>
            <h2>已登录账号</h2>
          </div>
        </div>
        {settings.loginAccounts.map((loginAccount) => {
          const accountUser = loginAccount.auth.user;
          const expanded = expandedLoginId === loginAccount.id;
          const active = settings.activeLoginId === loginAccount.id;
          return (
            <article className={`loginPanel ${expanded ? "isExpanded" : ""}`} key={loginAccount.id}>
              <div className="loginPanelTop">
                <button
                  className="loginPanelHeader"
                  onClick={() => toggleLoginPanel(loginAccount.id)}
                  type="button"
                >
                  <span className="miniIcon">
                    <UserRound size={16} />
                  </span>
                  <span className="loginPanelTitle">
                    <strong>{accountUser?.displayName || accountUser?.username || loginAccount.name}</strong>
                    <span>
                      {loginAccount.auth.baseUrl} · {accountUser?.group || "默认组"}
                    </span>
                  </span>
                  {active ? <span className="activeAccountBadge">当前</span> : null}
                  <ChevronDown size={17} className={expanded ? "chevronOpen" : ""} />
                </button>
                <div className="loginPanelActions">
                  <button className="ghostButton" onClick={() => activateLoginAccount(loginAccount.id)} type="button" disabled={active}>
                    设为当前
                  </button>
                  <button className="ghostButton dangerText" onClick={() => deleteLoginAccount(loginAccount.id)} type="button">
                    <Trash2 size={15} /> 删除账号
                  </button>
                </div>
              </div>
              {expanded ? <div className="loginPanelBody">{remoteKeySearchCard(loginAccount)}</div> : null}
            </article>
          );
        })}
      </section>
    );
  }

  function remoteKeySearchCard(loginAccount: LoginAccount) {
    return (
      <section className="card keySection">
        <div className="sectionHead">
          <div>
            <h2>远程 KEY</h2>
          </div>
          <button className="ghostButton" onClick={() => syncAllRemoteKeys(loginAccount)} type="button" disabled={!remoteKeys.length}>
            <RefreshCw size={14} /> 全部同步到本地 KEY 管理
          </button>
        </div>
        <div className="searchRow">
          <span className="searchField">
            <Search size={16} />
            <input
              placeholder="搜索 KEY 名称"
              value={remoteKeyword}
              onChange={(event) => setRemoteKeyword(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !remoteKeysLoading) void searchRemoteKeys(false, loginAccount.id);
              }}
            />
          </span>
          <button
            className="primaryButton"
            onClick={() => searchRemoteKeys(false, loginAccount.id)}
            type="button"
            disabled={remoteKeysLoading}
          >
            {remoteKeysLoading ? "查询中" : "查询"}
          </button>
        </div>
        <div className="remoteList">
          {remoteKeysLoading ? <div className="empty loadingBox">正在查询 KEY...</div> : null}
          {!remoteKeysLoading && remoteKeys.length === 0 ? <div className="empty">暂无远端 KEY</div> : null}
          {remoteKeys.map((item) => {
            const localId = remoteAccountId(item, loginAccount);
            const active = settings.activeAccountId === localId;
            const imported = settings.accounts.some((a) => a.id === localId);
            const busy = decryptingId === item.id;
            return (
              <article className={`remoteItem ${active ? "active" : ""}`} key={item.id || item.name}>
                <div className="miniIcon">
                  <KeyRound size={18} />
                </div>
                <div className="remoteTitle">
                  <strong>{item.name || "未命名 KEY"}</strong>
                  <span>
                    {item.group || "-"} <em>{statusText(item.status)}</em>
                  </span>
                </div>
                <code>{maskKey(item.apiKey)}</code>
                <button
                  className="ghostButton"
                  onClick={() => importRemoteKey(item, false, loginAccount)}
                  type="button"
                  disabled={!item.id || busy || imported}
                >
                  {imported ? "已同步到本地" : busy ? "解密中" : "同步到本地 KEY 管理"}
                </button>
                <button
                  className={`useButton ${active ? "active" : ""}`}
                  onClick={() => importRemoteKey(item, true, loginAccount)}
                  type="button"
                  disabled={!item.id || busy || active}
                >
                  <span className="useDot" />
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
        <header className="pageHead">
          <h1>技能商店</h1>
        </header>
        <section className="card placeholder">脚本市场和扩展推荐入口预留。</section>
      </>
    );
  }

  function settingsView() {
    const codexBusy = codexLaunchState === "starting" || codexLaunchState === "restarting";
    const skippedLockedRolloutFiles = Array.isArray(sessionRepairResult?.skippedLockedRolloutFiles)
      ? sessionRepairResult.skippedLockedRolloutFiles
      : [];
    return (
      <>
        <header className="pageHead">
          <h1>设置</h1>
        </header>

        {/* 版本更新 */}
        <section className="settingsGroup">
          <h2 className="groupTitle">版本更新</h2>
          <div className="groupCard">
            <div className="settingRow versionRow">
              <div className="settingLabel">
                <h3>codex助手</h3>
                {updateInfo?.updateAvailable ? (
                  <p className="updateHint">发现新版本 {updateInfo.latestVersion ? `v${trimVersionPrefix(updateInfo.latestVersion)}` : ""}</p>
                ) : null}
              </div>
              <div className="versionInfo">
                <span className="versionBadge">本地版本 v{appVersion || updateInfo?.currentVersion || "-"}</span>
                <span className={`versionBadge ${updateInfo?.updateAvailable ? "hasUpdate" : ""}`}>
                  最新版本 {updateInfo?.latestVersion ? `v${trimVersionPrefix(updateInfo.latestVersion)}` : "未检查"}
                </span>
                <button className="ghostButton" onClick={checkAppUpdate} type="button" disabled={updateChecking}>
                  <RefreshCw size={14} className={updateChecking ? "spin" : ""} />
                  {updateChecking ? "检查中" : "检查更新"}
                </button>
                {updateInfo?.updateAvailable ? (
                  <button
                    className="primaryButton"
                    onClick={installLatestUpdate}
                    type="button"
                    disabled={updateInstalling || !updateInfo.assetUrl}
                    title={updateInfo.assetUrl ? "下载新版本安装包" : "当前版本没有安装包下载地址"}
                  >
                    {updateInstalling ? (
                      <>
                        <RefreshCw size={14} className="spin" /> 下载中
                      </>
                    ) : (
                      <>
                        <Download size={14} /> 下载新版本
                      </>
                    )}
                  </button>
                ) : null}
              </div>
            </div>
            {updateInfo?.releaseSummary || updateInfo?.assetName ? (
              <div className="versionDetails">
                {updateInfo.assetName ? <span>安装包：{updateInfo.assetName}</span> : null}
                {updateInfo.releaseSummary ? <pre>{updateInfo.releaseSummary}</pre> : null}
              </div>
            ) : null}
          </div>
        </section>

        {/* Codex 运行 */}
        <section className="settingsGroup">
          <h2 className="groupTitle">Codex 运行</h2>
          <div className="groupCard">
            <div className="settingRow">
              <div className="settingLabel">
                <h3>启动 Codex</h3>
              </div>
              <div className="settingActions">
                <button
                  className={codexLaunchState === "started" ? "successButton" : "primaryButton"}
                  onClick={launchCodex}
                  type="button"
                  disabled={codexLaunchState === "starting" || codexLaunchState === "started"}
                >
                  {codexLaunchState === "starting" ? (
                    <>
                      <RefreshCw size={15} className="spin" /> 启动中
                    </>
                  ) : codexLaunchState === "started" ? (
                    "已启动"
                  ) : (
                    <>
                      <Play size={15} /> 启动
                    </>
                  )}
                </button>
                <button className="ghostButton" onClick={() => restartCodex()} type="button" disabled={codexBusy}>
                  <RotateCcw size={15} className={codexLaunchState === "restarting" ? "spin" : ""} />
                  {codexLaunchState === "restarting" ? "重启中" : "重启"}
                </button>
              </div>
            </div>

            <div className="settingRow pathRow">
              <div className="settingLabel">
                <h3>Codex 位置</h3>
                <p title={codexPath?.executablePath || codexPath?.message || ""}>
                  {codexPathChecking
                    ? "检测中..."
                    : codexPath?.status === "ok"
                      ? `${codexPath.source || "auto"}${codexPath.version ? ` · ${codexPath.version}` : ""}`
                      : codexPath?.message || "未检测到"}
                </p>
              </div>
              <button className="ghostButton" onClick={detectCodexPath} type="button" disabled={codexPathChecking}>
                <RefreshCw size={14} className={codexPathChecking ? "spin" : ""} /> {codexPathChecking ? "检测中" : "重新检测"}
              </button>
            </div>

            <div className="settingRow">
              <div className="settingLabel">
                <h3>安装 Codex</h3>
              </div>
              <button className="ghostButton" onClick={installCodex} type="button">
                安装
              </button>
            </div>
          </div>
        </section>

        {/* 增强 */}
        <section className="settingsGroup">
          <h2 className="groupTitle">增强</h2>
          <div className="groupCard">
            {/* 语言和默认指令 — 全宽独立区块 */}
            <div className="settingRow preferenceRow">
              <div className="preferenceHeader">
                <h3>语言和默认指令</h3>
                <div className="settingActions">
                  <button
                    className="ghostButton"
                    onClick={() =>
                      setCodexPreferenceForm((current) => ({
                        ...current,
                        developerInstructions: defaultChineseInstructions,
                      }))
                    }
                    type="button"
                  >
                    默认中文回复
                  </button>
                  <button className="primaryButton" onClick={saveCodexPreferences} type="button" disabled={codexPreferenceSaving || codexBusy}>
                    <RefreshCw size={15} className={codexPreferenceSaving || codexLaunchState === "restarting" ? "spin" : ""} />
                    重启codex生效
                  </button>
                </div>
              </div>
              <div className="preferenceGrid">
                <label>
                  <span>界面语言</span>
                  <select
                    value={codexPreferenceForm.localeOverride}
                    onChange={(event) => {
                      const localeOverride = event.currentTarget.value;
                      setCodexPreferenceForm((current) => ({
                        ...current,
                        localeOverride,
                      }));
                    }}
                  >
                    <option value="">自动检测</option>
                    <option value="zh-CN">中文（中国）</option>
                    <option value="en-US">English (US)</option>
                  </select>
                </label>
                <label>
                  <span>默认指令</span>
                  <textarea
                    value={codexPreferenceForm.developerInstructions}
                    onChange={(event) => {
                      const developerInstructions = event.currentTarget.value;
                      setCodexPreferenceForm((current) => ({
                        ...current,
                        developerInstructions,
                      }));
                    }}
                    placeholder="例如：请始终使用简体中文回答，除非我明确要求使用其他语言。"
                  />
                </label>
              </div>
            </div>

            {/* 解锁插件 */}
            <div className="settingRow">
              <div className="settingLabel">
                <h3>解锁插件</h3>
                <p>启用后会自动重启 Codex</p>
              </div>
              <button
                aria-label="解锁插件"
                aria-pressed={settings.pluginEnabled}
                className={`switch ${settings.pluginEnabled ? "isOn" : ""}`}
                disabled={codexBusy}
                onClick={togglePlugin}
                type="button"
              >
                <span />
              </button>
            </div>

            {/* 修复历史会话 */}
            <div className="settingRow sessionRepairRow">
              <div className="settingLabel">
                <h3>修复历史会话</h3>
                <p>切换账号或 API 模式后让旧会话重新可见。</p>
                {sessionRepairResult ? (
                  <div className="repairSummary">
                    <span>目标：{sessionRepairResult.targetProvider || "openai"}</span>
                    <span>会话文件：{sessionRepairResult.changedSessionFiles}</span>
                    <span>索引行：{sessionRepairResult.sqliteRowsUpdated}</span>
                    <span>跳过占用：{skippedLockedRolloutFiles.length}</span>
                    {sessionRepairResult.backupDir ? <span title={sessionRepairResult.backupDir}>备份：{sessionRepairResult.backupDir}</span> : null}
                    {sessionRepairResult.encryptedContentWarning ? (
                      <strong>{sessionRepairResult.encryptedContentWarning}</strong>
                    ) : null}
                  </div>
                ) : null}
              </div>
              <button className="ghostButton" onClick={repairHistoricalSessions} type="button" disabled={sessionRepairing}>
                <RefreshCw size={15} className={sessionRepairing ? "spin" : ""} />
                {sessionRepairing ? "修复中" : "立刻修复"}
              </button>
            </div>
          </div>
        </section>

        {/* 配置 — 合并为一行 */}
        <section className="settingsGroup">
          <h2 className="groupTitle">配置</h2>
          <div className="groupCard">
            <div className="settingRow">
              <div className="settingLabel">
                <h3>配置文件</h3>
              </div>
              <div className="settingActions">
                <button className="ghostButton" onClick={openConfigModal} type="button">
                  <FileText size={15} /> 查看
                </button>
                <button className="ghostButton" onClick={openRestorePreview} type="button" disabled={!configView?.backupAvailable}>
                  <RotateCcw size={15} /> 恢复
                </button>
              </div>
            </div>
          </div>
        </section>
      </>
    );
  }

  function updateStatusText() {
    if (updateChecking) return "正在检查 GitHub Release 最新版本...";
    if (!updateInfo) return "显示当前本地版本，可手动检查是否有新版本。";
    if (updateInfo.status === "failed") return updateInfo.message;
    if (updateInfo.updateAvailable) {
      return `发现新版本 ${updateInfo.latestVersion ? `v${trimVersionPrefix(updateInfo.latestVersion)}` : ""}`;
    }
    return "当前已是最新版本。";
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
            <button className="ghostButton" onClick={() => setConfigModalOpen(false)} type="button">
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
            <button className="ghostButton" onClick={() => setRestoreModalOpen(false)} type="button" disabled={restoreLoading}>
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
            <button className="ghostButton" onClick={() => setRestoreModalOpen(false)} type="button" disabled={restoreLoading}>
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
  const clean = baseUrl.replace(/\/+$/, "");
  if (!clean) return "";
  return clean.endsWith("/v1") ? clean : `${clean}/v1`;
}

function trimVersionPrefix(value: string) {
  return value.trim().replace(/^[vV]/, "");
}

function remoteAccountId(item: RemoteToken, loginAccount: LoginAccount) {
  return `remote-${loginAccount.id}-${item.id}`;
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
