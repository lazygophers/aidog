# 浮窗渲染性能优化 — PRD (主入口)

## 目标
- [ ] 托盘浮窗「经常卡顿没有及时展示」。两类主因: (A) 首帧延迟 —— `commands_tray/popover.rs:37-52` 5 个 await 全串行 + 内嵌 `tray_layout`(tray.rs:64) 循环每平台 `get_platform` N+1 查询 + 与 popover.rs:44 today_stats 重复查询; 前端 stats_query_batch 串在 popover_data resolve 之后(两跳串行, popover.tsx:75-94)。(B) 渲染卡顿/闪 —— 数据分 3-4 批到达, 每批触发 setSize+重定位, applySize 每次 3 IPC(popover.tsx:127-168), ResizeObserver 无节流 → 窗口肉眼连跳。目标: 首帧更快 + 消除 resize thrash。

## 边界
- [ ] 范围内(全 5 项):
  1. 后端 `popover_data` 用 `tokio::join!` 并发 5 个无依赖 await (popover.rs:37-52)。
  2. `tray_layout` 复用已查 today_stats(消一次重复聚合) + per-platform get_platform 改批量 `WHERE id IN (...)`(消 N+1)。tray_layout 是 tray 菜单+popover 共享函数, 改动优先内部复用而非改签名(memory high-freq-path-min-diff); 若必须改签名先 grep 两处调用点。
  3. 前端 config 缓存 localStorage(复用 popover.tsx:34-45 既有机制), 弹出时用缓存 config 立即 queryBatch 与 popover_data 并行; popover_data 回来若 config.items 变则补查。
  4. applySize 用 requestAnimationFrame 合并 ResizeObserver 多次触发(popover.tsx:127-168); 两次 outerPosition(146/155)可合一。
  5. renderGrid `useMemo`(popover.tsx:177) + CostTrendChart pts/path `useMemo`(CostTrendChart.tsx:28-36)。
- [ ] 范围外: 不改浮窗数据语义/卡片内容/布局结构; 不动窗口 prebuild+预 mount(app_setup.rs 冷启动已优化, 非本次瓶颈); 不加新 Tauri command。
- [ ] 约束: tray_layout 共享函数改动面控制(min diff); config 缓存不一致靠补查兜底; rAF 合并纯优化无行为变更。

## 验收标准
- [ ] popover.rs 5 await 并发化; tray_layout 消 N+1 + 复用 today_stats; 前端 config 缓存并行 batch; applySize rAF 合并; renderGrid+CostTrendChart memo; `cargo build` + `cargo clippy`(warning 清) + `yarn build` 全过; 弹出无肉眼连跳。

## 索引
- [ ] 任务/子任务/调度: task.json (`skein subtask list popover-render-perf`)
