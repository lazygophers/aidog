# Skills 模块 UI 修订 (agent 图标化 + scope 默认全局)

## Goal

对已交付的 Skills 管理模块做 3 点 UI/范围修订：agent 仅留 claude+codex 且改 SVG 图标激活态（去下拉），scope 默认只展示全局、选项目后才展示该项目 skills。

## Requirements

### R1 — agent 只支持 claude + codex（去 cursor）
- **后端** `gateway/skills.rs`：`SkillAgent` 枚举删 `Cursor` 变体 + `cli_name()` / `config_dir()` 对应臂 + 相关 test 断言（保留 claude/codex 断言）。
- **前端** `services/api.ts`：`SkillAgent` 类型 → `"claude" | "codex"`。
- **前端** `Skills.tsx`：`AGENTS = ["claude", "codex"]`，文件头注释同步。

### R2 — agent 选择改 SVG 图标激活/未激活态（非下拉）
- 移除 agent 的 `<select>`（Skills.tsx:235-247），换成一排可点 SVG 图标按钮（claude / codex 各一）。
- 图标资源复用现有：claude → `src/assets/platforms/claude_code.svg`，codex → `src/assets/platforms/openai.svg`（按现有 platform svg 的 import/img 用法）。
- 激活态（当前选中 agent）与未激活态视觉区分：激活高亮（边框/背景/不透明度），未激活灰/低透明；点击切换 `setAgent`。
- 保持 Liquid Glass 风格；无障碍：按钮带 `title`/`aria-label`（用 i18n agent 名）。

### R3 — scope 默认全局，选项目后才展示项目 skills
- 保留 scope 筛选控件（即"筛选的地方"），默认 `global` 展示全局已装 skills（现状已是，确认不回归）。
- 仅当用户在筛选处切到 project 并选定目录后，已装列表才展示该项目 skills（现状逻辑已满足：`scopeInvalid` 时不加载）。
- 若实现与该语义有偏差则修正；catalog（浏览/搜索）维持全局不变。

## Acceptance Criteria

- [ ] 后端 `SkillAgent` 无 Cursor，cargo test 绿（删/改对应断言）
- [ ] 前端 SkillAgent 类型 + AGENTS 仅 claude/codex
- [ ] agent 选择为 SVG 图标行，激活/未激活态可视区分，点击切换生效
- [ ] 默认进入页展示全局已装；切 project 选目录后才展示项目 skills
- [ ] i18n 无新增裸 key（复用 skills.agent.claude/codex，删 cursor key 可选）
- [ ] cargo clippy 无新 warning + cargo test 绿 + yarn build 绿

## Definition of Done

- cargo clippy 无 warning + cargo test 绿；yarn build 绿
- 8 locale 一致（删 cursor key 则 8 端同步删）
- 改动落 worktree，闭环 check→commit→archive

## Technical Approach

- 单 deliverable 轻量：main 在 worktree 内直接改（3 文件 + i18n）。
- agent 图标按钮组件可内联 Skills.tsx；import svg 用现有 platform svg 模式（`import xxx from "../assets/platforms/xxx.svg"` → `<img src=... />`）。

## Out of Scope

- 新增 cursor 之外 agent
- catalog 数据源/回退链改动
- scope 控件形态大改（保持现有 select 作筛选，仅确认语义）

## Technical Notes

- 现状：`Skills.tsx` AGENTS(23) / agent select(235-247) / scope select(224-232) / scopeInvalid(50) / loadInstalled deps(90)
- `skills.rs` SkillAgent(49) cli_name/config_dir(55-72) tests(521-523)
- 图标资源：`src/assets/platforms/{claude_code,openai}.svg`
- 参考 [[skills-management-module]]
