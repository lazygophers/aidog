---
title: decision-documentation
layer: recall
category: skein
keywords: [planning,execution,hypothesis-testing,decision-logging,design-vs-reality]
status: active
protected: true
---

## 实测推翻设计假设时的处理范式（留痕+不硬凑）

当 task 执行过程中发现「planning 写的验收文本与 exec 实测结果矛盾」时，按以下范式处理：

**模式**（logs-query-ipc-slimming s5 案例）：

设计文档说「Stats 页逐条日志事件全量重拉，需加 500ms debounce 节流」。
实测发现 `src/services/api/proxy.ts:113` 的 `onProxyLogUpdated(callback, debounceMs = 500)` 已内建 trailing-edge debounce。
三条验收标准（不再逐条重拉、节流实现复用既有 idiom、最终仍会更新）全部满足。

**做法**：
1. **留痕在 research 文档**（`research/s5-stats-throttle.md`）：
   - 小标题「前提证伪，无改动交付」
   - 清楚叙述发现（已内建 debounce 的代码位置 + line number）
   - 注明「三条验收在现状下已全部满足，故 done 但零改动」
   - 若有涉及 API 约束跨页影响的观察项，作为「遗留观察」记录（供下游 task 参考）

2. **勿硬凑原验收文本**（不逆向改改设计文档以符合现状 — 让记录反映真实）

3. **若设计假设重要**：添加 AskUserQuestion，让用户对「发现已内建」的事实拍板确认

**本源意义**：
- 捕捉真实的「预期 vs 现实」偏差，为后续类似 task 消灭掉重复假设
- 防止掩盖产品实现中的既有优化（如 debounce）被后续改动意外移除
- 建立明确的决策痕迹，而非讳莫如深
