# 修 dependabot npm 漏洞 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 清 lazygophers/aidog dependabot open alerts 中可修的 npm 传递漏洞。tar (critical GHSA-23hp-3jrh-7fpw ≤7.5.18 需 7.5.19 / high / medium) 在 root yarn.lock + docs/yarn.lock (via node-gyp@12.4.0 ^7.5.4, range 接受修复版); js-yaml (high GHSA-52cp-r559-cp3m <3.15.0 需 3.15.0) 在 docs/yarn.lock (via gray-matter ^3.13.1, range 接受)。yarn4 recursive up 重解析即可,无需改 package.json。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: (1) root `yarn up -R tar`; (2) docs/ `yarn up -R tar js-yaml`。仅动 yarn.lock + docs/yarn.lock。
- [x] 范围外: glib (rust GHSA-wrw7-89jp-8q8g <0.20.0) — 链 glib0.18.5→atk→gtk0.18.2→libappindicator→tray-icon→tauri2.11.5,gtk0.18 硬 pin glib0.18,本层不可升(需 tauri 升 gtk),且 gtk 仅 linux 编译 macOS 不受影响,soundness 非 RCE → 不在本 task,单列由 main 与用户定决策(dismiss/等 tauri)。不改 package.json 版本约束(range 已接受)。
- [x] 约束: yarn4 (berry) `yarn up -R` 递归升传递依赖; worktree 需先 yarn install。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [x] root yarn.lock tar version ≥7.5.19; docs/yarn.lock tar ≥7.5.19 + js-yaml ≥3.15.0; root `yarn build` 过; docs `yarn build`(或 docs:build) 过。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list dependabot-npm-bump`)
