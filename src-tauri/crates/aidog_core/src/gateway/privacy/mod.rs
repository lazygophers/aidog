//! 出站隐私脱敏引擎（纯函数层）。
//!
//! 检测请求文本中的机密与个人信息，替换为确定性占位符
//! `{{PII_<TYPE>_<8hex>}}`；`restore` 把占位符换回原值。
//! 映射表只在内存，原值不进 `Debug` / `Display` 输出。

mod detect;
mod engine;
mod model;
mod wire;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt;

pub use wire::{
    engine_unavailable, get_privacy_scrub_settings, privacy_gate, restore_json_escaped,
    scrub_json, scrub_json_with_model, scrub_options, set_engine_unavailable, PrivacyGate,
    PrivacyScrubSettings, StreamRestorer,
};

pub use model::refresh_model_gate;

/// 敏感数据类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PiiKind {
    /// API key（sk- 等常见前缀）。
    ApiKey,
    /// PEM 私钥块。
    PrivateKey,
    /// `Authorization: Bearer <token>`。
    BearerToken,
    /// 邮箱。
    Email,
    /// 银行卡号（Luhn 校验通过才算）。
    BankCard,
    /// 中国大陆 18 位身份证号（含校验位校验）。
    IdNumber,
}

impl PiiKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PiiKind::ApiKey => "API_KEY",
            PiiKind::PrivateKey => "PRIVATE_KEY",
            PiiKind::BearerToken => "BEARER_TOKEN",
            PiiKind::Email => "EMAIL",
            PiiKind::BankCard => "BANK_CARD",
            PiiKind::IdNumber => "ID_NUMBER",
        }
    }

    /// 机密类（可被利用的凭据），其余为个人信息类。
    pub fn is_secret(self) -> bool {
        matches!(
            self,
            PiiKind::ApiKey | PiiKind::PrivateKey | PiiKind::BearerToken
        )
    }
}

/// 类型级开关：机密类 / 个人信息类两档 + 单类型细粒度覆盖。
#[derive(Debug, Clone)]
pub struct ScrubOptions {
    /// 机密类总开关。
    pub secrets: bool,
    /// 个人信息类总开关。
    pub personal: bool,
    /// 单类型覆盖（优先于两档总开关）。
    pub per_kind: HashMap<PiiKind, bool>,
}

impl ScrubOptions {
    /// 全类型开启。
    pub fn all() -> Self {
        Self {
            secrets: true,
            personal: true,
            per_kind: HashMap::new(),
        }
    }

    /// 某类型是否启用。
    pub fn enabled(&self, kind: PiiKind) -> bool {
        if let Some(&on) = self.per_kind.get(&kind) {
            return on;
        }
        if kind.is_secret() {
            self.secrets
        } else {
            self.personal
        }
    }
}

impl Default for ScrubOptions {
    fn default() -> Self {
        ScrubOptions::all()
    }
}

/// 脱敏映射表：占位符 → 原值。原值不进 `Debug` / `Display`。
pub struct ScrubMap {
    inner: HashMap<String, String>,
}

impl ScrubMap {
    fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    fn insert(&mut self, placeholder: &str, original: &str) {
        self.inner
            .insert(placeholder.to_string(), original.to_string());
    }

    /// 占位符 → 原值，未登记返回 `None`。
    pub fn get(&self, placeholder: &str) -> Option<&str> {
        self.inner.get(placeholder).map(String::as_str)
    }

    /// 已登记的脱敏条目数。
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.inner.iter()
    }
}

impl fmt::Debug for ScrubMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(
                self.inner
                    .iter()
                    .map(|(p, o)| (p, format!("[REDACTED {}B]", o.len()))),
            )
            .finish()
    }
}

pub use engine::{restore, scrub};

