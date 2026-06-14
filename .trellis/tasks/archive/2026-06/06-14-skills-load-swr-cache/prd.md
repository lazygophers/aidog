# skills 页加载提速 — SWR 缓存

## Goal

消除 skills 页每次开页的卡顿。根因：每次 mount 跑 3 个 node 子进程（`checkEnv` 探 node+npx + `listInstalled` 跑 `npx skills list --json`，npx 冷启动 1-3s）。改为 **stale-while-revalidate**：开页瞬间显示缓存（含重启后），后台异步跑 npx 刷新，好了再更新 UI。写操作仍走 npx + 后失效缓存。

## What I already know（已核实）

- `src/pages/Skills.tsx`: mount 两个 useEffect —— `:60 skillsApi.checkEnv()` + `:75 skillsApi.listInstalled(scope)`（`loadInstalled` 带 setLoading 全屏等待）。
- `src-tauri/src/gateway/skills.rs`: `list_installed` shell out `npx skills list --json [-g]`；`check_env`(`:118`) spawn node `--version` + npx `--version` 两子进程。
- 写操作 enable/disable/update/uninstall/uninstallAll/enableAll/alignAgents 均 shell out npx（写约束不变）。
- 缓存模式参考刚做的 DbCache（写时失效）。

## Requirements（MVP）

- [ ] **后端 list 缓存 + 磁盘持久化**：`list_installed` 结果按 scope（global / project-path）缓存，持久化到磁盘（如 `~/.aidog/skills-cache.json`），跨重启可即时返回。
- [ ] **SWR 命令拆分**：
  - `skills_list_installed(scope)` → 立即返回缓存（命中即 0 子进程）；冷启动无缓存时返回空 + 标记 stale。
  - `skills_list_refresh(scope)` → 强制跑 npx、更新缓存+磁盘、返回 fresh。
- [ ] **checkEnv 进程内缓存**：`check_env` 结果缓存（node/npx 可用性一会话内不变），仅首次探测。
- [ ] **写操作后失效**：enable/disable/update/uninstall/uninstallAll/enableAll/alignAgents 成功后失效对应 scope 缓存（后端失效 + 前端触发一次 refresh）。
- [ ] **前端 SWR 流程**：mount → `skills_list_installed`(即时渲染缓存) → 后台 `skills_list_refresh`(不阻塞，仅小 "刷新中" 指示) → 完成更新列表。冷启动（无缓存）才显加载态。

## Acceptance Criteria

- [ ] 二次/重启后开 skills 页**瞬间显示**（缓存命中，无 npx 等待）；后台刷新完成后数据一致。
- [ ] 冷启动（首次无缓存）仍能正确加载（显加载态 → npx → 填充 + 落盘）。
- [ ] 写操作（启用/停用/更新/卸载）后列表正确反映变更（缓存失效 + 刷新生效，无脏数据）。
- [ ] scope 切换（global ↔ project）缓存不串（按 scope key）。
- [ ] `cargo build` + `cargo clippy`(无新 warning) + `cargo test` 全过；`yarn build` + `yarn check:i18n` 全过。
- [ ] 写仍走 npx（不破"写走 npx"约束）；不引入手动 fs 写 skill。

## Out of Scope

- list 改直读 fs 绕 npx（用户保留"list 走 npx"语义，只加缓存层）。
- 缓存 TTL 自动过期（本轮靠写操作失效 + 后台刷新，不做定时过期）。

## Technical Notes

- 缓存 key = scope 序列化（Global / Project{path}）。
- 磁盘缓存格式含 `cached_at` + per-scope items；读时容错（损坏/缺失 → 当冷启动）。
- "刷新中"指示走 i18n（如 `skills.refreshing`），7 语言补 key。
- 新文案过 `yarn check:i18n`。
- worktree: `.trellis/worktrees/06-14-skills-load-swr-cache`。
