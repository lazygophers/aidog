//! 代理设置 → npm/npx 代理 URL 构造 + 子进程代理 env 注入。

use crate::gateway::models::ProxyClientSettings;
use std::process::Command;

/// 由上游代理设置构造 npm/npx 用的代理 URL。
///
/// - 未启用（`enabled == false`）→ `None`（保持直连，不注入 env）。
/// - 启用 → `Some("{scheme}://[user:pass@]host:port")`。
/// - scheme：`socks5` 且 `dns_over_proxy` → `socks5h`（DNS 走代理）；否则按 proxy_type
///   映射（`socks5`/`https`/其余 → `http`），与 `ProxyClientSettings::to_reqwest_proxy` 一致。
///
/// ⚠️ socks5 限制：npm/npx 原生对 socks5 支持有限，依赖底层（如 undici / global-agent）的
/// `ALL_PROXY` 识别，未必所有 npm 版本生效；http/https 代理走 `HTTP_PROXY`/`HTTPS_PROXY` 最稳。
///
/// ⚠️ 认证编码：user/pass 原样嵌入 URL，不做 percent-encode。若凭证含 `@` `:` `/` 等保留字符，
/// 生成的 URL 可能被 npm/node 解析歧义（同 npm 自身约定：env 代理 URL 的凭证需调用方自行编码）。
/// 与 `to_reqwest_proxy`（用 `proxy.basic_auth` 内部处理）的差异仅在此边界场景显现。
pub fn proxy_env_url(settings: &ProxyClientSettings) -> Option<String> {
    if !settings.enabled {
        return None;
    }
    let scheme = match settings.proxy_type.as_str() {
        "socks5" if settings.dns_over_proxy => "socks5h",
        "socks5" => "socks5",
        "https" => "https",
        _ => "http",
    };
    let auth = if settings.username.is_empty() {
        String::new()
    } else {
        format!("{}:{}@", settings.username, settings.password)
    };
    Some(format!(
        "{}://{}{}:{}",
        scheme, auth, settings.host, settings.port
    ))
}

/// 为 npx `Command` 注入代理 env（若 `proxy_url` 为 `Some`）。
///
/// 设大小写两组 `HTTP_PROXY`/`HTTPS_PROXY`（兼容不同 npm/node 读法）；socks5(h) 时额外设
/// `ALL_PROXY`（npm 对 socks5 仅经此识别）。`None` → 不注入，保持直连行为不变。
pub(super) fn apply_proxy_env(cmd: &mut Command, proxy_url: Option<&str>) {
    let Some(url) = proxy_url else {
        return;
    };
    cmd.env("HTTP_PROXY", url)
        .env("HTTPS_PROXY", url)
        .env("http_proxy", url)
        .env("https_proxy", url);
    if url.starts_with("socks5") {
        cmd.env("ALL_PROXY", url).env("all_proxy", url);
    }
}

#[cfg(test)]
#[path = "test_proxy_env.rs"]
mod test_proxy_env;
