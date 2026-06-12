# StatusLine 段原子化

## Goal

把 StatusLine 每个原始信息原子化成独立段——用户可单独使用任一 Claude Code statusline 输入字段；保留少量便捷组合段，但**每个原始数据都必须有对应原子段**。**向后兼容现有段/配置**。

## Decisions

- **保留现有段不破坏**（model / context-bar / context-pct / git / cost / rate-limits / effort / vim / custom / group-*）作为「便捷组合」或已原子段，配置不回退。
- **新增全套原子段**覆盖研究枚举的所有字段。
- 缺失字段统一优雅降级（jq `// empty` / 条件判断 → 段输出空，复用 group 段的 `exit 0` 降级 + 全局分隔符已处理空段不留孤立分隔符）。

## Research

- 完整输入 schema + 拆分方案见 `research/statusline-input-schema.md`（40+ 字段，官方文档 https://code.claude.com/docs/en/statusline，v2.1.90+）。

## Requirements

### 新增原子段（R1，editors.tsx SEGMENT_DEFS）
覆盖以下字段，每个一段（key 用语义化、避免与现有冲突）：
- **成本/执行**：`cost-usd`(cost.total_cost_usd) / `session-duration`(total_duration_ms) / `api-duration`(total_api_duration_ms) / `lines-changed`(added/removed 合并便捷段)。
- **上下文**：`context-tokens`(total_input/output_tokens) / `context-max`(context_window_size) / `context-remaining`(remaining_percentage) / `context-cache`(current_usage.cache_creation/read_input_tokens)。（context-pct/context-bar 已有保留。）
- **速率限制**：`rate-limit-5h`(five_hour.used_percentage[+resets_at]) / `rate-limit-7d`(seven_day...)。（现 rate-limits 组合保留。）
- **Git**：`git-host`(repo.host) / `git-owner`(repo.owner) / `git-repo`(repo.name) / `git-repo-full`(owner/name 便捷)。（现 git 段保留。）
- **目录/会话**：`cwd`(workspace.current_dir) / `project-dir`(workspace.project_dir) / `added-dirs`(workspace.added_dirs) / `session-id` / `session-name` / `transcript-path`(可选)。
- **Worktree**：`worktree-name` / `worktree-branch` / `worktree-original-branch` / `git-worktree`(workspace.git_worktree)。
- **PR**：`pr-number`(pr.number) / `pr-url`(pr.url) / `pr-state`(pr.review_state)。
- **其他单字段**：`version` / `output-style`(output_style.name) / `thinking`(thinking.enabled) / `token-warn`(exceeds_200k_tokens) / `agent`(agent.name)。`effort`/`vim` 已有保留。
- `model` 已有（display_name/id via format）保留。

### 段实现（R2）
- R2.1 每段 SEGMENT_DEFS 项：type/name/icon/desc/defaultOptions/toBash(jq)/toPreview(mock)/fields。
- R2.2 toBash 缺失字段优雅降级：用 `// empty` 或 `[ -z "$v" ] && exit 0`，缺字段段输出空（不报错、不留孤立分隔符——现 generateStatusLineScript 已捕获空段处理）。
- R2.3 可选 options：如百分比/原始值、ms→人类可读(s/min)、token 缩写(formatNumber 思路 bash 内)、resets_at 显剩余时间(可选)。合理默认。
- R2.4 段融入现有颜色/autoColor(值类段可上色,如 context/rate/cost 按阈值)/对齐/全局分隔符/多行体系；段选择器列出（分组归类更好，可选）。
- R2.5 文案走 i18n t()，补 7 语言（段 name/desc，复用 statusline.seg.<type>.* 命名）。

### 原始脚本回退模式（R4，与原子段同属 StatusLinePanel，合并实施避免 editors.tsx 冲突）
- R4.1 StatusLine / SubagentStatusLine 各加「使用内置结构化 vs 自定义脚本」模式开关。
- R4.2 **未启用内置**时，允许用户填原始脚本地址/命令（Claude Code 原生 statusLine 支持）：写入 settings 的 `statusLine` / `subagentStatusLine` 为原生格式（如 `{ "type": "command", "command": "<path-or-cmd>" }`，对照 Claude Code statusLine 配置格式），不生成 aidog 脚本。
- R4.3 启用内置时维持现有结构化段生成行为。模式切换不互相破坏；两种配置互斥（启用内置则忽略自定义脚本，反之）。
- R4.4 文案 t() 补 7 语言。

### 不回退（R5）
- R5.1 现有段（含 group-*）类型/行为不变；旧配置正常渲染。
- R5.2 新段与现有体系（颜色/对齐/分隔符/多行/preview）兼容。
- R5.3 现有内置结构化生成路径不回退。

## Acceptance Criteria

- [ ] 研究枚举的每个原始字段都有对应原子段可单独选用。
- [ ] 保留便捷组合段（context-bar / git / rate-limits / cost / lines-changed / git-repo-full）。
- [ ] 各新段生成 bash 正确（jq 提取 + 缺失降级 exit 0），bash -n 通过。
- [ ] 值类段支持 autoColor/固定色；兼容对齐/全局分隔符/多行/preview。
- [ ] 旧配置/现有段零回退；段选择器列出全部新段。
- [ ] tsc 0；7 语言段文案补齐。

## Definition of Done

- 全部原子段落地 + 必要便捷组合；缺失字段不报错不留孤立分隔符。
- 不破坏现有段/group 段/配置；jq 表达式与官方字段路径一致。
- tsc 0；i18n 7 语言。

## Out of Scope

- 不去后端/不改 group-info 端点。
- 不改全局分隔符/颜色/对齐既有机制（仅新增段接入）。
- resets_at 倒计时复杂格式可简化（显百分比为主，剩余时间可选/降级）。

## Technical Notes

- 文件：`src/components/settings/editors.tsx`（SEGMENT_DEFS / SegmentType 联合 / toBash / toPreview / fields / 段选择器），`src/locales/*.json`。
- 降级 + 分隔符空段处理已在 generateStatusLineScript（group-info 引入的 `__seg` 捕获机制）——新段复用。
- jq 字段路径严格对照 research/statusline-input-schema.md。
