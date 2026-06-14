# PRD: Skills 一键全启用某 agent

## 背景
Skills 页逐 skill per-agent toggle。用户想让某 agent(claude 或 codex)启用所有已装 skills,需逐条点。加快捷方式:一键为某 agent 启用全部已装 skills。

## 语义
对每条已装 skill — 若该 agent 未启用 → enable;已启用 → 跳过。只增不减(非破坏性)。

## 范围 (单交付, main worktree 内直接写)
限定 aidog SkillAgent (claude/codex)。

### 后端
- `skills.rs` 新增 `enable_all(agent, scope, proxy_url) -> SkillsOpResult`
  - `list_installed` 取当前 → 逐 skill:agent 未启用则 `enable()` → 聚合(enabled N)
  - stdout=`enabled N skills`;stderr=聚合错误
- `lib.rs` 新 command `skills_enable_all(db, agent, scope)` + 注册

### 前端
- `api.ts`: `skillsApi.enableAll(agent, scope)`
- `Skills.tsx`: **统计卡每 agent 计数块旁加"全部启用"小按钮**(非破坏性, 无需 modal/二次确认) → handler (调后端 → 刷新列表 → toast, busyKey=`__enableall_${agent}__`)

### i18n (8 语言)
`skills.enableAll` / `skills.enableAllDone` (含 {{agent}}{{count}}) / `skills.enableAllNoop` (含 {{agent}}) / `skills.enabling`

## 验证
- `cargo test gateway::skills` 绿
- `cargo clippy` 无 warning
- `yarn build` exit 0
- check-i18n 零缺失
- 实跑: 某 agent 部分启用 → 点全启用 → 该 agent 全启用

## 不做
- 不加 modal (非破坏性,统计卡按钮直接执行)
- 不扩展到 claude/codex 外 agent
- 不批量 npx (逐 skill spawn, N 小可接受, 与 align_agents 一致)
