//! Agent Skills 管理子系统。
//!
//! **全 npx 化**：list / enable / disable / update 全部 shell out `npx skills`，
//! 禁手动 fs 扫描 / 删除（复用 Vercel Labs 官方生态，单一事实源）。
//! - 列表：`npx skills list --json [-g]` → 统一一条/skill，`agents[]` 含显示名（"Claude Code"/"Codex"）= 该 agent 已启用。
//! - 启用：用 skill 本地 path（list json `path`）作 add package → `npx skills add <path> -a <slug> [-g] -y`（对所有 skill 通用，不依赖锁文件 source）。
//! - 关闭：`npx skills remove -s <name> -a <slug> [-g] -y`。
//! - 更新：`npx skills update [-g] -y`。
//!
//! 所有变更操作均 shell out npx，不违反"全 npx"约束。
//!
//! shell out 模式参考 `gateway/notification.rs`（`std::process::Command`）。
//!
//! Scope 语义：
//! - `Global` → 用户级全局，命令带 `-g`。
//! - `Project { path }` → 项目级，命令在项目目录内执行（不带 `-g`）。
//!
//! Agent 语义：target agent 决定 `-a <slug>` 参数（claude → `claude-code`、codex → `codex`）
//! 与 list json `agents[]` 显示名映射（claude → "Claude Code"、codex → "Codex"）。
//!
//! ── 模块划分（结构搬移，行为不变）──
//! - `types`     数据模型 + 枚举 + 常量。
//! - `env`       npx/node 探测 + home env 注入。
//! - `proxy_env` 代理 URL 构造 + 子进程代理 env 注入。
//! - `npx`       `npx skills <args>` 执行封装 + scope→cwd 路由。
//! - `list`      list_installed + JSON/frontmatter/锁文件解析。
//! - `cache`     list 的 SWR 缓存（进程内 + 磁盘）。
//! - `catalog`   browse/search/find 及输出解析。
//! - `ops`       单 skill 写操作（enable/install/disable/update/uninstall）+ fs 兜底删。
//! - `bulk`      批量写操作（align_agents/enable_all）。
//! - `detail`    详情只读浏览（文件树 + 单文件读取）。

mod bulk;
mod cache;
mod catalog;
mod detail;
mod env;
mod list;
mod npx;
mod ops;
mod proxy_env;
mod types;

// ── 对外路径保持 `gateway::skills::X` 不变（re-export）──

pub use types::{
    CatalogEntry, SkillAgent, SkillDetail, SkillFileContent, SkillScope, SkillsEnv, SkillsOpResult,
};
// SkillFile / SkillInfo 是 SkillDetail / CachedSkills 的字段类型（经 serde 序列化），
// 无外部按名引用但保持公开可达，对齐拆分前 `gateway::skills::X` 公共面。
#[allow(unused_imports)]
pub use types::{SkillFile, SkillInfo};

pub use bulk::{align_agents, enable_all};
pub use cache::{invalidate, list_cached, list_refresh, CachedSkills};
pub use catalog::{browse_catalog, search};
pub use detail::{detail, read_file};
pub use env::check_env;
pub use list::list_installed;
pub use ops::{disable, enable, install, uninstall, uninstall_all, update};
pub use proxy_env::proxy_env_url;
