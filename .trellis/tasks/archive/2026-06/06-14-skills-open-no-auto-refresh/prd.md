# skills 开页纯缓存不自动刷新

## Goal

消除 skills 页「还慢」的感知。真因：现 `loadInstalled` 每次开页在缓存渲染后**强制后台 `listRefresh`**（npx 冷启 1.34s 实测）+ 置「刷新中」+ 列表跳变。改为**开页纯缓存渲染、不自动 refresh**，npx 仅在必要时跑。保持 npx 约束不变（用户已确认不松绑）。

## What I already know（已核实）

- `npx skills list --json` 冷启 **1.34s**（实测，不可压缩地板）。
- `~/.aidog/skills-cache.json` 已生效（后端缓存工作正常）。
- `src/pages/Skills.tsx`: `loadInstalled`(`:88`) 缓存渲染后**无条件** `await skillsApi.listRefresh(scope)`；`useEffect`(`:120`) mount/scope 变即调。`refreshInstalled`(`:71`) 已是独立 force refresh 函数。后端 `skills_list_installed`(缓存) / `skills_list_refresh`(npx) 已拆分；写操作后端已 invalidate。

## Requirements（MVP）

- [ ] **开页 = 纯缓存**：mount/scope 切换时只调 `skills_list_installed`（缓存，0 npx），命中即渲染，**不自动跟 listRefresh**。
- [ ] **冷启动兜底**：缓存 stale（无缓存）时才跑一次 `listRefresh` 填充（显加载态）。
- [ ] **写操作后刷新不变**：enable/disable/update/uninstall/uninstallAll/enableAll/alignAgents 后仍 force `refreshInstalled`（保证变更可见）。
- [ ] **显式「刷新」按钮**：页面加一个手动刷新按钮（调 `refreshInstalled`，带「刷新中」态），供用户在「外部改了 skills」时主动拉取最新。i18n key `skills.refresh` 7+ 语言。
- [ ] 移除开页时的自动「刷新中」指示（仅手动刷新/写操作时显示）。

## Acceptance Criteria

- [ ] 二次/重启开 skills 页**纯缓存秒开**，无 npx、无 spinner、无列表跳变（cargo build 后实测开页 0 子进程，仅冷启动 1 次）。
- [ ] 冷启动（删 skills-cache.json 后首开）正确加载 + 落盘。
- [ ] 写操作后列表正确反映变更（force refresh 生效）。
- [ ] 手动刷新按钮能拉取外部改动。
- [ ] `yarn build` + `node scripts/check-i18n.mjs` 全过（新 `skills.refresh` key 7+ locale 齐）。

## Out of Scope

- list 改直读 fs（用户保留 npx 约束）。
- 缓存 TTL 自动过期。
- 后端改动（缓存层已就绪，仅前端调用策略变 + 一个 i18n key）。

## Technical Notes

- 仅改 `src/pages/Skills.tsx`（load 流程 + 刷新按钮）+ `src/locales/*.json`（skills.refresh）。
- `refreshInstalled` 已存在，复用即可。
- worktree: `.trellis/worktrees/06-14-skills-open-no-auto-refresh`。
