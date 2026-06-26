# skills 列表读全局锁文件免 npx

## 背景

用户原话 (2026-06-25): 「我希望 skills 的列表信息（已安装的、全局的）直接从 `~/.agents/.skill-lock.json` 读取，不经过 npx skills 命令，但是搜索、安装、移除、更新、启用/禁用还是需要经过命令的。会全局的也还是通过命令」

## 范围澄清 (用户明确界定)

| 操作 | 数据源 | 改动 |
|---|---|---|
| **列表读取** (已装) | **直接读 `~/.agents/.skill-lock.json`** (免 npx, 快) | ✅ 改 |
| **搜索** | npx `skills find` | 不改 |
| **安装 / 移除 / 更新** | npx 命令 | 不改 |
| **启用 / 禁用** (写) | npx 命令 | 不改 |
| **全局目录浏览** | npx `find` (catalog) | 不改 |

## 现状

- `src-tauri/src/gateway/skills/list.rs:27` `list_installed` 走 `npx skills list --json` shell out (慢)
- npx `list --json` 返 `[{name, path, scope, agents:[...]}]`, `agents[]` 含某 agent 显示名 = 该 agent 已启用 → 映射 `enabled_agents` (claude/codex)
- `~/.agents/.skill-lock.json` 结构 (version 3):
  ```json
  { "version": 3, "skills": { "<name>": {
      "source": "owner/repo", "sourceType": "github",
      "sourceUrl": "https://github.com/owner/repo.git",
      "skillPath": "skills/<name>/SKILL.md",
      "skillFolderHash": "<sha1>",
      "pluginName"?: "<plugin>",
      "installedAt": "ISO", "updatedAt": "ISO"
  }}}
  ```
  **无 agents[] 字段** → per-agent enable 状态锁文件不记录, 需另取

## enable 状态源 (用户已裁定: 探测本地 symlink)

- **claude enable** = `~/.claude/skills/<name>` symlink 存在 (→ `~/.agents/skills/<name>`, 已验证)
- **codex enable** = 待研究 (`~/.codex/` 无 skills 目录; 可能走 `~/.codex/AGENTS.md` 或 `~/.codex/config.toml` 引用, 或 codex 不支持 symlink 启用)
- agent 须先研究 codex skills 启用落点 (npx skills enable -a codex 实际写哪), 再定探测方式

## 需求 (实现目标)

1. `list_installed` 改为读 `~/.agents/.skill-lock.json` 解析为 `Vec<SkillInfo>` (免 npx spawn)
2. per-agent enable 状态: 探测本地文件系统 (claude symlink + codex 待研究机制), 不走 npx
3. `enrich_with_sources` 现走锁文件富化 — 锁文件成主数据源后简化 (字段直接来自锁文件)
4. 保持 `(items, ok)` 返回签名 (SWR 链路 `cache::list_refresh` 依赖 ok 区分真空 vs 加载失败)
5. scope 语义: Project scope 读锁文件 + 探测 project 本地 skills 目录? (锁文件是全局的) — agent 研究锁文件是否区分 scope, 不区分则 Project scope 行为待定 (可能仍需 npx 或只读全局)

## 关键约束

- memory [[skills-management-module]] [[npx-skills-cli]] [[skills-catalog-dual-source]] [[skills-claude-code-home-env]]
- `~/.agents/.skill-lock.json` 读路径经 `resolve_home_env` (memory [[skills-claude-code-home-env]]: HOME env 异常致漏检, spawn npx 须 apply_home_env; 纯文件读同样需 dirs::home_dir)
- 保持 `align_agents` / `enable_all` 内部聚合仍能取实时态 (这些是写操作前的对齐读, 可保留 npx 或改锁文件, agent 定)
- 不破坏 cache 层 (`list_cached` / `list_refresh` SWR 契约)

## 字段展示需求 (用户 2026-06-26 反馈扩展)

用户反馈: 「~/.agents/.skill-lock.json 的一些其它的信息没有展示在 skills 的列表页」

锁文件独有字段 (npx list --json 不返, 现状列表页未展示) **须透出到前端**:

| 锁文件字段 | 展示建议 |
|---|---|
| `source` (owner/repo) | 列表卡显示来源仓库 |
| `sourceType` (github/...) | 类型标签 |
| `sourceUrl` (git url) | 可点击跳转 / 复制 |
| `skillFolderHash` | 调试用, 可折叠/详情区 |
| `pluginName` (可选, plugin 安装才有) | plugin 来源标记 |
| `installedAt` (ISO) | 安装时间, 格式化显示 |
| `updatedAt` (ISO) | 更新时间, 格式化显示 |

实现:
- `SkillInfo` (types.rs) 补上述字段 (serde 从锁文件反序列化), 保持 camelCase 兼容前端
- 前端 `Skills.tsx` / `services/api.ts` TS 类型同步 + 列表卡渲染这些字段 (用 `utils/formatters.ts` 格式化时间, 禁页内重复定义)
- i18n: 新增展示字段 label 走 8 locale 全覆盖 (check-i18n 把关)
- 布局: 不破坏现有卡片结构, 新字段用紧凑次要信息行 (CompactCard / StatChip 风格, memory [[shared-ui-formatters]])

## 验收

- `list_installed` 零 npx spawn (grep `run_npx_in_scope` 在 list 路径无调用)
- **锁文件 7 字段全部透出前端展示** (source/sourceType/sourceUrl/skillFolderHash/pluginName/installedAt/updatedAt)
- 列表加载明显变快 (无 shell out 延迟)
- enable 状态正确反映 symlink 存在性 (claude 确认; codex 研究后确认)
- cargo test 全绿 (list 相关单测改 mock 锁文件 + tempdir)
- cargo clippy 0 warning
- yarn build + check-i18n 全绿

## 风险

- codex enable 探测机制不明 → 可能需 fallback npx 或 config 解析 (TOML)
- Project scope 语义: 锁文件全局共享, Project scope 列表可能需不同源
- 锁文件 schema 变更 (version 3 → 4) 兼容性
