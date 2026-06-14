# skills 按 owner/repo 来源层级分组展示

## Goal

Skills 页扁平列表 → **按 skill 来源（owner/repo）分组的层级展示**。同一 owner/repo 下的多个 skill 折到一个组头下，组内仍是 per-item agent 切换行。第三方/手动 symlink skill（无锁文件 source）单独归「手动安装」组。

用户决策（AskUserQuestion）：
- 新建层级 task（与 active `skills-open-no-auto-refresh` 并列，顺序执行避免 Skills.tsx 冲突）。
- 「项目」= owner/repo 来源（锁文件 `source` 字段）。
- 组折叠默认 = **全展开**。
- 无 source 组名 = **「其他」**（i18n key `skills.groupOther`）。
- 组头 **加组级 agent 批量操作**（一键全组启用/禁用某 agent）。

## 根因 / 现状（已核实）

- `SkillInfo`（`skills.rs:93` / `api.ts:1286`）**无 source 字段** → 前端无法分组。
- `npx skills list --json` **不返回 source**（memory `npx-skills-cli`）。
- source 在锁文件 `~/.agents/.skill-lock.json`（global）/ `<project>/.agents/.skill-lock.json`（project）→ `{version, skills:{<name>:{source:"owner/repo", sourceType, sourceUrl, installedAt,...}}}`。
- 第三方/手动 symlink skill（如 understand-*，由外部应用 symlink 到 `~/.agents/skills/<name>`）**不在锁文件** → 无 source → 归「手动安装」组。
- 前端 `Skills.tsx` 列表渲染 `installed.map(...)`（`:163` 附近）扁平结构，需改为分组聚合 + 组头 + 折叠态。

## 数据源

- 后端新增 `read_skill_sources(scope) -> HashMap<String, String>`：读锁文件 `skills` map，`name → source`（owner/repo）。文件缺失/损坏 → 空 map（等价所有 skill 无 source = 全归「手动安装」组）。
- `SkillInfo` 加 `pub source: Option<String>`（owner/repo，无则 None）。
- `parse_list_json` / `list_installed` 填充：name 命中锁文件 source map → Some(source)；否则 None。
- 缓存 `SkillsCacheFile` 的 items 序列化时携带 source（缓存命中也带分组信息，0 子进程）。

## 分组规则

| skill source | 组 key | 组头显示 | 排序位 |
| --- | --- | --- | --- |
| `Some("owner/repo")` | `"owner/repo"` | `owner/repo` + skill 计数 | 按 owner/repo 字母升序 |
| `None`（无锁文件条目） | `"__other__"` 哨兵 | 「其他」+ 计数 | **置末** |
| 空 source（锁文件有条目但 source 空） | 同 None | 「其他」 | 置末 |

组内 skill 行按 name 字母升序。

## 组级 agent 批量操作

**后端新增 command** `skills_set_group_agent(group_source: Option<String>, agent: SkillAgent, enable: bool, scope: SkillScope)`:
- 读 list_installed → 过滤 `source == group_source`（None 组用 `Option::<String>::None` 表示，匹配 source=None 的 skill）。
- 逐个 enable/disable（npx 串行，复用 enable/disable 内部逻辑），汇总成功/失败计数 + 每条 stderr。
- 成功后 invalidate(scope)（与现有写命令一致）。
- 返回 `SkillsOpResult`（stdout 汇总 "X/Y skills updated"，stderr 聚合失败）。

**前端组头**: claude/codex 图标（与 per-item 同款 SVG），三态：
- 组内所有 skill 该 agent 都启用 → solid accent；点击 = **批量 disable 全组**。
- 部分 → grayscale 描边；点击 = **批量 enable 缺的**（已启用不动）。
- 全未启用 → outline；点击 = **批量 enable 全组**。
- busy 态禁并发（`busyKey = "group:<source>:<agent>"`），完成后 force refresh + 折叠态保持。

## UI

- **组头**：glass-elevated 卡，左 `owner/repo`（mono 字体）+ 右 skill 计数徽章；点击组名区折叠/展开（chevron 旋转）；右侧组级 claude/codex 图标（独立点击区，不触发折叠）。
- **折叠态**：默认**全展开**；折叠态仅显组头 + 计数 + 组级图标。每 scope 各自维护折叠 set（内存 `Set<string>`，不持久化）。
- **组内行**：保留现 per-item agent 切换行（claude/codex 图标 + skill 名），不变。
- **统计卡**（顶部大数字 + 每 agent 启用数）：仍按全局 installed 聚合，不按组。
- **i18n**：新增 `skills.groupOther`（「其他」）+ `skills.skillCount`（「{n} 个 skill」）+ `skills.groupUpdated`（「已更新 {x}/{y} 个 skill」）7 语言。

## Acceptance Criteria

- [ ] `SkillInfo.source` 后端 + TS 类型对齐（cross-layer-rules snake_case）。
- [ ] `read_skill_sources` 读锁文件，单测覆盖：正常 map / 文件缺失 / 损坏 JSON / source 空。
- [ ] list_installed / list_refresh 返回的 items 带 source；缓存序列化携带。
- [ ] `skills_set_group_agent` command + 单测（mock 不真跑 npx 写）。
- [ ] 前端按 source 分组渲染：组头 + 折叠（默认全展开）+ 组级 agent 批量图标 + 组内 per-item 行。
- [ ] 第三方 symlink skill（无锁文件条目）归「其他」组。
- [ ] per-item agent 切换 / 写操作刷新 / 手动刷新按钮行为不变。
- [ ] 组级批量三态图标 + 点击语义正确（全启→disable、部分/全未→enable 缺的）。
- [ ] `cargo clippy --all-targets` 0 项目 warning；`cargo test` 全过。
- [ ] `yarn build` + `node scripts/check-i18n.mjs` 全过（新 key 7 locale 齐）。

## Out of Scope

- source → catalog 元数据（描述/星标）展示（catalog 已移除，不回引）。
- 跨 scope 混合分组（global / project 各自独立列表，不变）。
- 折叠态持久化（本次仅内存 set；持久化后续 task）。

## 不做

- 不动 active `skills-open-no-auto-refresh` 的刷新逻辑（顺序执行，本 task 完成后再做或反之）。
- 不改 npx 命令构造 / uninstall fs 兜底。
