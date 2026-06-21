//! Skills 子系统数据模型与枚举（纯类型 + 小型 impl，零 IO）。

use serde::{Deserialize, Serialize};

/// 安装目标 scope。`Global` = 用户级全局；`Project` = 指定项目目录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SkillScope {
    /// 用户级全局（`-g`）。
    Global,
    /// 项目级，path 为项目根目录绝对路径。
    Project { path: String },
}

impl SkillScope {
    /// 缓存键：`Global` → `"global"`；`Project{path}` → `"project:<path>"`。
    /// 不同项目 path 不串；trim 后比较（与命令 cwd 一致）。
    pub(super) fn cache_key(&self) -> String {
        match self {
            SkillScope::Global => "global".to_string(),
            SkillScope::Project { path } => format!("project:{}", path.trim()),
        }
    }
}

/// 目标 agent。决定 `--agent` 参数与本地配置目录名。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillAgent {
    Claude,
    Codex,
}

impl SkillAgent {
    /// `npx skills ... -a <slug>` 的 agent slug。
    /// claude → `claude-code`（修正旧 "claude"）；codex → `codex`。
    pub(super) fn cli_slug(self) -> &'static str {
        match self {
            SkillAgent::Claude => "claude-code",
            SkillAgent::Codex => "codex",
        }
    }

    /// `npx skills list --json` 的 `agents[]` 显示名。用于解析某 agent 是否启用。
    pub(super) fn display_name(self) -> &'static str {
        match self {
            SkillAgent::Claude => "Claude Code",
            SkillAgent::Codex => "Codex",
        }
    }

    /// 目标 agent 全集（UI 仅显示 claude/codex 两个）。
    pub(super) fn all() -> [SkillAgent; 2] {
        [SkillAgent::Claude, SkillAgent::Codex]
    }
}

/// 环境探测结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsEnv {
    /// `npx` 是否可用（写操作前置）。
    pub npx_available: bool,
    /// `node --version` 输出（如 "v20.11.0"），不可用为 null。
    pub node_version: Option<String>,
}

/// 已装 skill 描述（`npx skills list --json` 解析产出，一条/skill，不分 agent）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// skill 名。
    pub name: String,
    /// 已在哪些目标 agent（claude/codex 子集）启用 —— 从 list json `agents[]` 显示名映射。
    pub enabled_agents: Vec<SkillAgent>,
    /// 所属 scope。
    pub scope: SkillScope,
    /// 规范存储路径（list json `path`），读不到为 null。
    pub installed_path: Option<String>,
    /// 简介（list json 暂无，预留；读不到为 null）。
    pub description: Option<String>,
    /// 来源 owner/repo（锁文件 `source` 字段）。第三方/手动 symlink skill（锁文件无条目）→ None。
    pub source: Option<String>,
}

/// catalog 条目（可装 skill）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// 安装标识（owner/repo 或 skill slug）。
    pub id: String,
    /// 展示名。
    pub name: String,
    /// 简介。
    pub description: Option<String>,
    /// 来源仓库 URL。
    pub repo_url: Option<String>,
}

/// 写操作（install/update/remove）结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsOpResult {
    /// 退出码为 0 视为成功。
    pub success: bool,
    /// 合并的 stdout。
    pub stdout: String,
    /// 合并的 stderr。
    pub stderr: String,
}

/// skill 详情视图：文件列表（只读浏览）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFile {
    /// 相对 skill 根的路径（`/` 分隔，跨平台统一）。
    pub rel_path: String,
    /// 字节数。
    pub size: u64,
    /// 启发式判定为文本文件（首块无 NUL）。
    pub is_text: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDetail {
    /// skill 名（目录 basename）。
    pub skill_name: String,
    /// canonicalized skill 根绝对路径。
    pub root: String,
    /// 文件列表（SKILL.md 置首，其余按路径字母序）。
    pub files: Vec<SkillFile>,
}

/// 单文件读取结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFileContent {
    /// 文本内容；二进制/读失败 → None。
    pub content: Option<String>,
    /// 超 MAX_READ_BYTES（512 KB）截断。
    pub truncated: bool,
    /// 原始字节数。
    pub size: u64,
}

/// 单文件读取上限（512 KB）；超出截断。
pub(super) const MAX_READ_BYTES: usize = 512 * 1024;
/// 二进制检测：读取前 N 字节判断是否含 NUL。
pub(super) const BINARY_SNIFF_BYTES: usize = 8192;

#[cfg(test)]
#[path = "test_types.rs"]
mod test_types;
