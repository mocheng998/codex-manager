use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use tungstenite::{Message, connect, stream::MaybeTlsStream};

const PLUGIN_UNLOCK_SCRIPT: &str = r#"
(() => {
  if (window.__codexManagerPluginUnlockInstalled) return;
  window.__codexManagerPluginUnlockInstalled = true;

  const selectors = {
    disabledInstallButton: 'button:disabled, button[aria-disabled="true"], [role="button"][aria-disabled="true"], button[data-disabled], [role="button"][data-disabled], button.cursor-not-allowed, [role="button"].cursor-not-allowed, button.pointer-events-none, [role="button"].pointer-events-none',
    pluginNavButton: 'nav[role="navigation"] button.h-token-nav-row.w-full',
    pluginSvgPath: 'svg path[d^="M7.94562 14.0277"]',
  };

  function reactFiberFrom(element) {
    const fiberKey = Object.keys(element).find((key) => key.startsWith("__reactFiber"));
    return fiberKey ? element[fiberKey] : null;
  }

  function authContextValueFrom(element) {
    for (let fiber = reactFiberFrom(element); fiber; fiber = fiber.return) {
      for (const value of [fiber.memoizedProps?.value, fiber.pendingProps?.value]) {
        if (value && typeof value === "object" && typeof value.setAuthMethod === "function" && "authMethod" in value) {
          return value;
        }
      }
    }
    return null;
  }

  function spoofChatGPTAuthMethod(element) {
    const auth = authContextValueFrom(element);
    if (!auth || auth.authMethod === "chatgpt") return;
    auth.setAuthMethod("chatgpt");
  }

  function pluginEntryButton() {
    const byIcon = document.querySelector(`${selectors.pluginNavButton} ${selectors.pluginSvgPath}`)?.closest("button");
    if (byIcon) return byIcon;
    return Array.from(document.querySelectorAll(selectors.pluginNavButton))
      .find((button) => /^(插件|Plugins)(\s+-\s+.*)?$/i.test((button.textContent || "").trim())) || null;
  }

  function labelUnlockedPluginEntry(button) {
    const labelTextNode = Array.from(button.querySelectorAll("span, div")).reverse()
      .flatMap((node) => Array.from(node.childNodes))
      .find((node) => node.nodeType === 3 && /^(插件|Plugins)( - 已解锁| - Unlocked)?$/i.test((node.nodeValue || "").trim()));
    if (!labelTextNode) return;
    const current = (labelTextNode.nodeValue || "").trim();
    labelTextNode.nodeValue = /^Plugins/i.test(current) ? "Plugins - Unlocked" : "插件 - 已解锁";
  }

  function patchReactDisabledProps(element) {
    Object.keys(element)
      .filter((key) => key.startsWith("__reactProps"))
      .forEach((key) => {
        const props = element[key];
        if (!props || typeof props !== "object") return;
        props.disabled = false;
        props["aria-disabled"] = false;
        props["data-disabled"] = undefined;
      });
  }

  function clearDisabledState(element) {
    if (!(element instanceof HTMLElement)) return;
    if ("disabled" in element) element.disabled = false;
    element.removeAttribute("disabled");
    element.removeAttribute("aria-disabled");
    element.removeAttribute("data-disabled");
    element.removeAttribute("inert");
    element.classList.remove("disabled", "opacity-50", "cursor-not-allowed", "pointer-events-none");
    element.style.pointerEvents = "auto";
    element.style.opacity = "";
    element.style.cursor = "pointer";
    element.tabIndex = 0;
    patchReactDisabledProps(element);
  }

  function enablePluginEntry() {
    const pluginButton = pluginEntryButton();
    if (!pluginButton) return;
    spoofChatGPTAuthMethod(pluginButton);
    clearDisabledState(pluginButton);
    pluginButton.style.display = "";
    pluginButton.querySelectorAll("*").forEach((node) => {
      node.style.display = "";
      clearDisabledState(node);
    });
    labelUnlockedPluginEntry(pluginButton);
    if (pluginButton.dataset.codexManagerPluginEnabled === "true") return;
    pluginButton.dataset.codexManagerPluginEnabled = "true";
    pluginButton.addEventListener("click", () => spoofChatGPTAuthMethod(pluginButton), true);
  }

  function installButtonCandidates() {
    const nodes = Array.from(document.querySelectorAll(selectors.disabledInstallButton));
    return Array.from(new Set(nodes.map((node) => node.closest?.("button, [role='button']") || node)));
  }

  function isInstallButtonLabel(text) {
    return /^安装\s*/.test(text) || /^Install\s*/i.test(text) || text === "强制安装";
  }

  function unlockInstallButton(button) {
    if (!isInstallButtonLabel((button.textContent || "").trim())) return;
    [button, ...(button.querySelectorAll?.("button, [role='button'], [disabled], [aria-disabled], [data-disabled], .cursor-not-allowed, .pointer-events-none") || [])]
      .forEach(clearDisabledState);
    const walker = document.createTreeWalker(button, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const node = walker.currentNode;
      if (isInstallButtonLabel((node.nodeValue || "").trim())) {
        node.nodeValue = "强制安装";
        break;
      }
    }
  }

  let refreshQueued = false;
  function refreshPluginUnlock() {
    refreshQueued = false;
    enablePluginEntry();
    installButtonCandidates().forEach(unlockInstallButton);
  }

  function queuePluginUnlockRefresh() {
    if (refreshQueued) return;
    refreshQueued = true;
    setTimeout(refreshPluginUnlock, 100);
  }

  refreshPluginUnlock();
  const observer = new MutationObserver(queuePluginUnlockRefresh);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  setInterval(refreshPluginUnlock, 1000);
})();
"#;

#[derive(Clone, Debug, Deserialize)]
struct CdpTarget {
    #[serde(rename = "type")]
    target_type: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: Option<String>,
}

pub fn plugin_unlock_arguments(debug_port: u16) -> [String; 2] {
    [
        format!("--remote-debugging-port={debug_port}"),
        format!("--remote-allow-origins=http://127.0.0.1:{debug_port}"),
    ]
}

pub fn spawn_plugin_unlock_injection(debug_port: u16) {
    std::thread::spawn(move || {
        for _ in 0..20 {
            if inject_plugin_unlock(debug_port).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

fn inject_plugin_unlock(debug_port: u16) -> anyhow::Result<()> {
    let targets = list_targets(debug_port)?;
    let target = pick_page_target(&targets)?;
    let websocket_url = target
        .web_socket_debugger_url
        .as_deref()
        .context("selected CDP target has no websocket URL")?;
    evaluate_script(websocket_url, PLUGIN_UNLOCK_SCRIPT, true)?;
    evaluate_script(websocket_url, PLUGIN_UNLOCK_SCRIPT, false)?;
    Ok(())
}

fn list_targets(debug_port: u16) -> anyhow::Result<Vec<CdpTarget>> {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .context("failed to build CDP HTTP client")?;
    let targets = client
        .get(format!("http://127.0.0.1:{debug_port}/json"))
        .send()
        .context("failed to query CDP targets")?
        .error_for_status()
        .context("CDP target query failed")?
        .json::<Vec<CdpTarget>>()
        .context("failed to deserialize CDP targets")?;
    Ok(targets)
}

fn pick_page_target(targets: &[CdpTarget]) -> anyhow::Result<&CdpTarget> {
    let mut first_page = None;
    for target in targets {
        if target.target_type != "page"
            || target
                .web_socket_debugger_url
                .as_deref()
                .unwrap_or("")
                .is_empty()
        {
            continue;
        }
        first_page.get_or_insert(target);
        let haystack = format!("{} {}", target.title, target.url).to_lowercase();
        if haystack.contains("codex") {
            return Ok(target);
        }
    }
    first_page.context("No injectable Codex page target found")
}

fn evaluate_script(websocket_url: &str, script: &str, new_document: bool) -> anyhow::Result<()> {
    let (mut socket, _) = connect(websocket_url).context("failed to connect CDP websocket")?;
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
    }

    let method = if new_document {
        "Page.addScriptToEvaluateOnNewDocument"
    } else {
        "Runtime.evaluate"
    };
    let params = if new_document {
        json!({ "source": script })
    } else {
        json!({
            "expression": script,
            "allowUnsafeEvalBlockedByCSP": true
        })
    };
    let id = if new_document { 1 } else { 2 };
    socket.send(Message::Text(
        json!({
            "id": id,
            "method": method,
            "params": params
        })
        .to_string(),
    ))?;

    loop {
        let message = socket.read()?;
        let Message::Text(text) = message else {
            continue;
        };
        let payload: Value = serde_json::from_str(&text)?;
        if payload.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(error) = payload.get("error") {
            bail!("CDP injection failed: {error}");
        }
        return Ok(());
    }
}
