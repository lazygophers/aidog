# PRD: Skills 对齐两 agent 配置快捷方式

## 背景
Skills 页每条 skill per-agent toggle 手动切换。用户有多 skill 时, 想让 codex 配置与 claude 完全一致 (或反之), 需逐条点。加快捷方式: 选源 agent + 目标 agent, 一键使目标配置与源完全一致。

## 语义
target 对齐 source: 对每条已装 skill —
- source 启用 + target 未启用 → enable(target)
- source 未启用 + target 启用 → disable(target)
- 其余不变

## 范围 (单交付, main worktree 内直接写)
限定 aidog SkillAgent (claude/codex) 之间对齐 (UI 管辖范围; 其他 13 agent 非 aidog 管)。

### 后端
- `skills.rs` 新增 `align_agents(from: SkillAgent, to: SkillAgent, scope, proxy_url) -> SkillsOpResult`
  - `list_installed(scope, proxy)` 取当前每 skill 的 enabled_agents
  - 逐 skill 比对 from/to → 调 `enable()`/`disable()`
  - 聚合: success=全成功; stdout=`aligned N changes (M enabled, K disabled)`; stderr=聚合错误
  - from==to → success=true, stdout="noop: source equals target"
- `lib.rs` 新 command `skills_align_agents(db, from, to, scope)` + 注册

### 前端
- `api.ts`: `skillsApi.alignAgents(from, to, scope)`
- `Skills.tsx`: header 第三个按钮 "对齐配置" → modal (from select / to select / 禁 from==to / 说明文案) → 确认 → handler (调后端 → 刷新列表 → toast, busyKey=`__align__`)

### i18n (8 语言)
`skills.alignTitle` / `skills.alignFrom` / `skills.alignTo` / `skills.alignConfirm` (含 {{from}}{{to}}) / `skills.alignDone` / `skills.alignNoop`

## 验证
- `cargo test gateway::skills` 绿 (加 align 单测: mock 比对逻辑?enable/disable 调 npx 难单测 → 测 from==to noop 分支 + 空 list)
- `cargo clippy` 无 warning
- `yarn build` exit 0
- check-i18n 零缺失
- 实跑: claude [a,b] + codex [a,c] → align claude→codex → codex [a,b]

## 不做
- 不扩展到 claude/codex 外 agent (YAGNI)
- 不做 dry-run 预览 (modal 文案已说明语义)
- 不批量 npx (逐 skill spawn, N 小可接受)
