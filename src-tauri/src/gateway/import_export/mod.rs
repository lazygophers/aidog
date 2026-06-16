//! 导入导出子系统。
//!
//! 把 aidog 的可配置数据（platform / group / setting / codex / claude-code
//! settings / skills）序列化为加密单文件 `.aidogx`，
//! 供用户跨机器迁移。文件密钥经字节混淆隐藏在头部，程序可重组、人眼无法分辨。
//!
//! 模块：
//! - [`container`]：`.aidogx` 二进制格式加解密。
//! - [`collect`]：导出 — 从 db + 文件系统收集各 scope 数据 → payload。
//! - [`apply`]：导入 — payload → db 写入 + 文件回写 + 冲突检测。
//! - [`skills_sync`]：skills 自动化（npx add/enable/disable）。

pub mod apply;
pub mod collect;
pub mod container;
pub mod skills_sync;

pub use container::{decrypt, encrypt};

use serde::{Deserialize, Serialize};

/// 导出 / 导入范围标识（前端勾选框 value 与此后端枚举字符串一致）。
pub const SCOPE_PLATFORM: &str = "platform";
pub const SCOPE_GROUP: &str = "group";
pub const SCOPE_GROUP_PLATFORM: &str = "group_platform";
pub const SCOPE_SETTING: &str = "setting";
pub const SCOPE_CODEX: &str = "codex";
pub const SCOPE_CLAUDE_CODE: &str = "claude_code";
pub const SCOPE_SKILLS: &str = "skills";

/// 命名文本对（group name → 文件内容）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedText {
    pub name: String,
    pub text: String,
}

/// 容器 manifest（元数据 + 完整性校验）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub aidog_version: String,
    pub created_at: String,
    pub source_machine: String,
    pub scopes: Vec<String>,
    /// SHA256(明文 payload JSON，本字段置空时算)。解密后必比对。
    pub checksum: String,
}

/// 完整 payload（明文 JSON，加密前）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payload {
    pub manifest: Manifest,
    #[serde(default)]
    pub platform: Vec<serde_json::Value>,
    #[serde(default)]
    pub group: Vec<serde_json::Value>,
    /// (group_name, platform_name) — 按名称存，跨机迁移友好。
    #[serde(default)]
    pub group_platform: Vec<[String; 2]>,
    /// (scope, key, value_json) — setting 表原始行。
    #[serde(default)]
    pub setting: Vec<[String; 3]>,
    /// `~/.codex/config.toml` 文本（None = 源机不存在）。
    #[serde(default)]
    pub codex_global: Option<String>,
    /// `{group}.config.toml` 各 profile。
    #[serde(default)]
    pub codex_profiles: Vec<NamedText>,
    /// `~/.claude/settings.json` 文本（None = 源机不存在）。
    #[serde(default)]
    pub claude_code_global: Option<String>,
    /// `~/.aidog/settings.{group}.json` 各 group。
    #[serde(default)]
    pub claude_code_group_settings: Vec<NamedText>,
    #[serde(default)]
    pub skills: Vec<skills_sync::SkillExportEntry>,
}

impl Payload {
    /// 序列化为 JSON 字节并填 manifest.checksum（SHA256 over 自身 checksum 置空版）。
    pub fn serialize_with_checksum(&mut self) -> Result<Vec<u8>, String> {
        self.manifest.checksum.clear();
        let bytes = serde_json::to_vec(self).map_err(|e| format!("serialize payload: {e}"))?;
        let hash = container::sha256_hex(&bytes);
        self.manifest.checksum = hash;
        // 再次序列化（checksum 已填）。
        serde_json::to_vec(self).map_err(|e| format!("serialize payload (final): {e}"))
    }

    /// 从 JSON 字节解析并校验 checksum。
    pub fn from_bytes_verified(bytes: &[u8]) -> Result<Self, String> {
        let mut payload: Payload =
            serde_json::from_slice(bytes).map_err(|e| format!("parse payload: {e}"))?;
        let stored = payload.manifest.checksum.clone();
        payload.manifest.checksum.clear();
        let rebytes =
            serde_json::to_vec(&payload).map_err(|e| format!("reserialize for checksum: {e}"))?;
        let calc = container::sha256_hex(&rebytes);
        if stored != calc {
            return Err(format!(
                "checksum mismatch: stored {stored} != calculated {calc}"
            ));
        }
        payload.manifest.checksum = stored;
        Ok(payload)
    }
}

/// 冲突项（前端弹窗逐项决策用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictItem {
    pub scope: String,
    /// 唯一键：platform/group=名称；setting=scope:key；
    /// codex/claude_code 文件=文件名。
    pub key: String,
    pub existing_summary: String,
    pub incoming_summary: String,
}

/// 导入预览（解密后返回前端，供冲突弹窗收集决策）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub manifest: Manifest,
    pub scopes: Vec<String>,
    pub conflicts: Vec<ConflictItem>,
    /// 各 scope 待导入条目数（信息展示用）。
    pub counts: std::collections::BTreeMap<String, usize>,
}

/// 单条冲突决策。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    Overwrite,
    Skip,
    Rename { new_key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictDecision {
    pub scope: String,
    pub key: String,
    pub decision: Decision,
}

/// 导入结果报告。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub applied: std::collections::BTreeMap<String, usize>,
    pub skipped: std::collections::BTreeMap<String, usize>,
    pub errors: Vec<String>,
}
