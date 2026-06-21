use super::*;
use crate::gateway::models::ProxyClientSettings;
use std::process::Command;

fn proxy_settings(
    enabled: bool,
    ty: &str,
    user: &str,
    pass: &str,
    dns_over_proxy: bool,
) -> ProxyClientSettings {
    ProxyClientSettings {
        enabled,
        proxy_type: ty.to_string(),
        host: "1.2.3.4".to_string(),
        port: 7890,
        username: user.to_string(),
        password: pass.to_string(),
        dns_over_proxy,
    }
}

#[test]
fn proxy_env_url_disabled_is_none() {
    let s = proxy_settings(false, "http", "", "", true);
    assert_eq!(proxy_env_url(&s), None);
}

#[test]
fn proxy_env_url_http_no_auth() {
    let s = proxy_settings(true, "http", "", "", true);
    assert_eq!(proxy_env_url(&s).as_deref(), Some("http://1.2.3.4:7890"));
}

#[test]
fn proxy_env_url_https_with_auth() {
    let s = proxy_settings(true, "https", "u", "p", true);
    assert_eq!(proxy_env_url(&s).as_deref(), Some("https://u:p@1.2.3.4:7890"));
}

#[test]
fn proxy_env_url_socks5_dns_over_proxy_is_socks5h() {
    let s = proxy_settings(true, "socks5", "", "", true);
    assert_eq!(proxy_env_url(&s).as_deref(), Some("socks5h://1.2.3.4:7890"));
}

#[test]
fn proxy_env_url_socks5_no_dns_is_socks5() {
    let s = proxy_settings(true, "socks5", "", "", false);
    assert_eq!(proxy_env_url(&s).as_deref(), Some("socks5://1.2.3.4:7890"));
}

#[test]
fn proxy_env_url_socks5_with_auth() {
    let s = proxy_settings(true, "socks5", "u", "p", false);
    assert_eq!(proxy_env_url(&s).as_deref(), Some("socks5://u:p@1.2.3.4:7890"));
}

#[test]
fn proxy_env_url_unknown_type_falls_back_http() {
    let s = proxy_settings(true, "weird", "", "", true);
    assert_eq!(proxy_env_url(&s).as_deref(), Some("http://1.2.3.4:7890"));
}

fn env_of<'a>(cmd: &'a Command, key: &str) -> Option<&'a std::ffi::OsStr> {
    cmd.get_envs()
        .find(|(k, _)| *k == std::ffi::OsStr::new(key))
        .and_then(|(_, v)| v)
}

#[test]
fn apply_proxy_env_none_injects_nothing() {
    let mut cmd = Command::new("npx");
    apply_proxy_env(&mut cmd, None);
    assert_eq!(cmd.get_envs().count(), 0);
}

#[test]
fn apply_proxy_env_http_sets_http_https_not_all() {
    let mut cmd = Command::new("npx");
    apply_proxy_env(&mut cmd, Some("http://1.2.3.4:7890"));
    assert_eq!(
        env_of(&cmd, "HTTP_PROXY"),
        Some(std::ffi::OsStr::new("http://1.2.3.4:7890"))
    );
    assert_eq!(
        env_of(&cmd, "HTTPS_PROXY"),
        Some(std::ffi::OsStr::new("http://1.2.3.4:7890"))
    );
    assert_eq!(
        env_of(&cmd, "http_proxy"),
        Some(std::ffi::OsStr::new("http://1.2.3.4:7890"))
    );
    // 非 socks5 → 不设 ALL_PROXY。
    assert_eq!(env_of(&cmd, "ALL_PROXY"), None);
}

#[test]
fn apply_proxy_env_socks5_also_sets_all_proxy() {
    let mut cmd = Command::new("npx");
    apply_proxy_env(&mut cmd, Some("socks5h://1.2.3.4:7890"));
    assert_eq!(
        env_of(&cmd, "ALL_PROXY"),
        Some(std::ffi::OsStr::new("socks5h://1.2.3.4:7890"))
    );
    assert_eq!(
        env_of(&cmd, "all_proxy"),
        Some(std::ffi::OsStr::new("socks5h://1.2.3.4:7890"))
    );
    assert_eq!(
        env_of(&cmd, "HTTP_PROXY"),
        Some(std::ffi::OsStr::new("socks5h://1.2.3.4:7890"))
    );
}
