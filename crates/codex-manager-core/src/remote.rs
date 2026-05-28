use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use reqwest::blocking::{Client, Response};
use reqwest::header::{
    ACCEPT, CACHE_CONTROL, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, ORIGIN, REFERER,
    SET_COOKIE, USER_AGENT,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::settings::{
    AuthState, AuthUser, DEFAULT_AUTH_BASE_URL, StoredCookie, clean_url, normalize_api_key,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginCredentials {
    #[serde(default = "default_login_mode")]
    pub login_mode: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default, alias = "account")]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPayload {
    pub auth: AuthState,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteKeySearchPayload {
    pub keyword: String,
    pub items: Vec<RemoteToken>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteKeyDecryptPayload {
    pub token_id: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteToken {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub group: String,
    pub status: Value,
    pub raw: Value,
}

pub fn login_new_api(credentials: LoginCredentials) -> anyhow::Result<LoginPayload> {
    if credentials.login_mode.trim() != "newApi" {
        bail!("only newApi login mode is supported");
    }
    let username = credentials.username.trim();
    if username.is_empty() || credentials.password.is_empty() {
        bail!("请输入账号和密码");
    }
    let base_url = normalized_auth_base_url(&credentials.base_url);
    let mut client = NewApiClient::new(base_url.clone())?;

    let _ = client.request(
        "GET",
        "/sign-in",
        build_headers([
            (
                ACCEPT.as_str(),
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
            (REFERER.as_str(), &format!("{base_url}/sign-in")),
        ]),
        None,
    )?;

    let login_payload = client.request_json(
        "POST",
        "/api/user/login?turnstile=",
        build_headers([
            (ACCEPT.as_str(), "application/json, text/plain, */*"),
            (CACHE_CONTROL.as_str(), "no-store"),
            (CONTENT_TYPE.as_str(), "application/json"),
            (ORIGIN.as_str(), &base_url),
            (REFERER.as_str(), &format!("{base_url}/sign-in")),
        ]),
        Some(json!({
            "username": username,
            "password": credentials.password,
        })),
    )?;
    if login_payload.get("success") == Some(&Value::Bool(false)) {
        bail!(
            extract_api_message(&login_payload)
                .unwrap_or_else(|| "用户名或密码错误，或用户已被禁用".to_string())
        );
    }

    let login_user =
        normalize_auth_user(login_payload.get("data")).context("登录成功，但没有拿到用户信息")?;
    let user_header = new_api_user_header(&login_user)?;
    let self_payload = client.request_json(
        "GET",
        "/api/user/self",
        build_headers([
            (ACCEPT.as_str(), "application/json, text/plain, */*"),
            (CACHE_CONTROL.as_str(), "no-store"),
            ("New-Api-User", &user_header),
            (
                REFERER.as_str(),
                &format!("{base_url}/sign-in?redirect=%2Fkeys"),
            ),
        ]),
        None,
    )?;
    if self_payload.get("success") != Some(&Value::Bool(true)) || self_payload.get("data").is_none()
    {
        bail!(
            extract_api_message(&self_payload)
                .or_else(|| extract_api_message(&login_payload))
                .unwrap_or_else(|| "登录校验失败".to_string())
        );
    }
    let user =
        normalize_auth_user(self_payload.get("data")).context("登录成功，但没有拿到用户信息")?;

    Ok(LoginPayload {
        auth: AuthState {
            login_mode: "newApi".to_string(),
            base_url,
            user: Some(user),
            cookies: client.cookies,
            updated_at_ms: now_ms(),
        },
    })
}

pub fn search_remote_keys(
    auth: &AuthState,
    keyword: &str,
) -> anyhow::Result<RemoteKeySearchPayload> {
    let user = auth.user.as_ref().context("请先登录后再查询远程 KEY")?;
    let base_url = normalized_auth_base_url(&auth.base_url);
    let user_header = new_api_user_header(user)?;
    let mut client = NewApiClient::with_cookies(base_url.clone(), auth.cookies.clone())?;
    let keyword = keyword.trim().to_string();
    let referer = if keyword.is_empty() {
        format!("{base_url}/keys")
    } else {
        format!("{base_url}/keys?filter={}", url_encode(&keyword))
    };
    let payload = client.request_json(
        "GET",
        &format!("/api/token/search?keyword={}", url_encode(&keyword)),
        build_headers([
            (ACCEPT.as_str(), "application/json, text/plain, */*"),
            (CACHE_CONTROL.as_str(), "no-store"),
            ("New-Api-User", &user_header),
            (REFERER.as_str(), &referer),
        ]),
        None,
    )?;
    Ok(RemoteKeySearchPayload {
        keyword,
        items: normalize_remote_tokens(&payload),
    })
}

pub fn decrypt_remote_key(
    auth: &AuthState,
    token_id: &str,
) -> anyhow::Result<RemoteKeyDecryptPayload> {
    let user = auth.user.as_ref().context("请先登录后再解密远程 KEY")?;
    let token_id = token_id.trim();
    if token_id.is_empty() {
        bail!("缺少远程 KEY ID");
    }
    let base_url = normalized_auth_base_url(&auth.base_url);
    let user_header = new_api_user_header(user)?;
    let mut client = NewApiClient::with_cookies(base_url.clone(), auth.cookies.clone())?;
    let payload = client.request_json(
        "POST",
        &format!("/api/token/{}/key", url_encode(token_id)),
        build_headers([
            (ACCEPT.as_str(), "application/json, text/plain, */*"),
            (CACHE_CONTROL.as_str(), "no-store"),
            ("New-Api-User", &user_header),
            (ORIGIN.as_str(), &base_url),
            (REFERER.as_str(), &format!("{base_url}/keys")),
        ]),
        None,
    )?;
    let api_key = extract_remote_key_value(&payload)
        .map(normalize_api_key)
        .filter(|value| !value.is_empty())
        .context("接口已返回成功，但没有拿到 KEY 内容")?;
    Ok(RemoteKeyDecryptPayload {
        token_id: token_id.to_string(),
        api_key,
    })
}

struct NewApiClient {
    base_url: String,
    client: Client,
    cookies: Vec<StoredCookie>,
}

impl NewApiClient {
    fn new(base_url: String) -> anyhow::Result<Self> {
        Self::with_cookies(base_url, Vec::new())
    }

    fn with_cookies(base_url: String, cookies: Vec<StoredCookie>) -> anyhow::Result<Self> {
        Ok(Self {
            base_url,
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()?,
            cookies,
        })
    }

    fn request_json(
        &mut self,
        method: &str,
        path: &str,
        headers: HeaderMap,
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let response = self.request(method, path, headers, body)?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        let payload = if text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or_else(|_| json!({ "message": text }))
        };
        if !status.is_success() {
            bail!(
                "{}",
                extract_api_message(&payload).unwrap_or_else(|| format!("请求失败：{status}"))
            );
        }
        Ok(payload)
    }

    fn request(
        &mut self,
        method: &str,
        path: &str,
        headers: HeaderMap,
        body: Option<Value>,
    ) -> anyhow::Result<Response> {
        let url = format!("{}{}", self.base_url, path);
        let mut builder = match method {
            "POST" => self.client.post(&url),
            _ => self.client.get(&url),
        };
        builder = builder.headers(headers);
        if !self.cookies.is_empty() {
            builder = builder.header("Cookie", cookie_header(&self.cookies));
        }
        if let Some(body) = body {
            builder = builder.json(&body);
        }
        let response = builder.send()?;
        merge_set_cookie(&mut self.cookies, response.headers());
        Ok(response)
    }
}

fn normalized_auth_base_url(value: &str) -> String {
    let base_url = clean_url(value);
    if base_url.is_empty() {
        DEFAULT_AUTH_BASE_URL.to_string()
    } else {
        base_url
    }
}

fn build_headers<const N: usize>(entries: [(&str, &str); N]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
    );
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"));
    headers.insert("Connection", HeaderValue::from_static("keep-alive"));
    for (key, value) in entries {
        if let (Ok(key), Ok(value)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            headers.insert(key, value);
        }
    }
    headers
}

fn normalize_auth_user(value: Option<&Value>) -> Option<AuthUser> {
    let raw = value?.as_object()?;
    let username = raw.get("username")?.as_str()?.trim().to_string();
    if username.is_empty() {
        return None;
    }
    let display_name = raw
        .get("display_name")
        .or_else(|| raw.get("displayName"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&username)
        .to_string();
    Some(AuthUser {
        id: raw.get("id").and_then(Value::as_u64).unwrap_or_default(),
        username,
        display_name,
        group: raw
            .get("group")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        role: raw.get("role").and_then(Value::as_i64).unwrap_or_default(),
        status: raw
            .get("status")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
    })
}

fn new_api_user_header(user: &AuthUser) -> anyhow::Result<String> {
    if user.id == 0 {
        bail!("当前登录态缺少用户 ID，无法请求远程 KEY");
    }
    Ok(user.id.to_string())
}

fn normalize_remote_tokens(payload: &Value) -> Vec<RemoteToken> {
    let list = payload
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| {
            payload
                .get("data")
                .and_then(|data| data.get("items"))
                .and_then(Value::as_array)
        })
        .or_else(|| payload.get("items").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default();

    list.into_iter()
        .filter_map(|raw| {
            let object = raw.as_object()?;
            Some(RemoteToken {
                id: string_field(object, &["id", "token_id", "key_id"]),
                name: string_field(object, &["name"]),
                api_key: string_field(object, &["key", "masked_key"]),
                group: string_field(object, &["group", "token_group"]),
                status: object
                    .get("status")
                    .or_else(|| object.get("enabled"))
                    .cloned()
                    .unwrap_or(Value::Null),
                raw,
            })
        })
        .collect()
}

fn string_field(object: &serde_json::Map<String, Value>, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| object.get(*key))
        .map(|value| match value {
            Value::String(text) => text.trim().to_string(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

fn extract_remote_key_value(payload: &Value) -> Option<String> {
    for value in [
        payload.get("data"),
        payload.get("key"),
        payload.get("token"),
        payload.get("value"),
        payload.get("api_key"),
        payload.get("apiKey"),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(text) = value
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            return Some(text.to_string());
        }
        if let Some(object) = value.as_object() {
            for key in ["key", "token", "value", "api_key", "apiKey"] {
                if let Some(text) = object
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                {
                    return Some(text.to_string());
                }
            }
        }
    }
    None
}

fn extract_api_message(payload: &Value) -> Option<String> {
    ["message", "error"].into_iter().find_map(|key| {
        payload
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })
}

fn merge_set_cookie(cookies: &mut Vec<StoredCookie>, headers: &HeaderMap) {
    for value in headers.get_all(SET_COOKIE).iter() {
        let Ok(line) = value.to_str() else {
            continue;
        };
        let Some(cookie) = parse_set_cookie(line) else {
            continue;
        };
        cookies.retain(|entry| {
            !(entry.name == cookie.name
                && entry.domain == cookie.domain
                && entry.path == cookie.path)
        });
        cookies.push(cookie);
    }
}

fn parse_set_cookie(line: &str) -> Option<StoredCookie> {
    let mut segments = line
        .split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty());
    let name_value = segments.next()?;
    let (name, value) = name_value.split_once('=')?;
    let mut cookie = StoredCookie {
        name: name.trim().to_string(),
        value: value.to_string(),
        domain: String::new(),
        path: "/".to_string(),
        secure: false,
        http_only: false,
        same_site: "lax".to_string(),
        expiration_date: None,
    };
    for segment in segments {
        let (key, value) = segment.split_once('=').unwrap_or((segment, ""));
        match key.trim().to_ascii_lowercase().as_str() {
            "domain" => cookie.domain = value.trim().to_string(),
            "path" => cookie.path = value.trim().to_string(),
            "secure" => cookie.secure = true,
            "httponly" => cookie.http_only = true,
            "samesite" => cookie.same_site = value.trim().to_ascii_lowercase(),
            "max-age" => {
                if let Ok(seconds) = value.trim().parse::<i64>() {
                    cookie.expiration_date = Some((now_ms() / 1000) as i64 + seconds);
                }
            }
            _ => {}
        }
    }
    Some(cookie)
}

fn cookie_header(cookies: &[StoredCookie]) -> String {
    cookies
        .iter()
        .filter(|cookie| !cookie.name.is_empty())
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect::<Vec<_>>()
        .join("; ")
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn default_login_mode() -> String {
    "newApi".to_string()
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
