# 浮窗性能优化 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 浮窗(tray popover)点击后经常卡顿、没及时展示。根因: 每次点托盘全新建 webview 窗口(冷启动) + 失焦即 `destroy()`，且 4 路 IPC 瀑布(popover_data/queryBatch/groupApi.list/groupDetailApi.list)后才首帧。目标: 消除冷启延迟，点即显。用户选定「全量」力度: 窗口复用 + 数据保温 + 精简 resize/重定位 + 复核 backdrop-filter 模糊开销。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: (1) Rust 窗口生命周期改造 — 启动预建隐藏 popover 窗口，tray toggle 改 show/hide + 重定位，失焦 Focused(false) 由 destroy 改 hide (app_setup.rs + startup.rs)；(2) 前端数据保温 — 复用窗口后 React state/data 跨开自然常驻，show 时先展上次数据再后台刷 (popover.tsx)；(3) 精简 resize/重定位异步循环 (减少 outerPosition/scaleFactor/setSize/setPosition 往返)；(4) 复核 backdrop-filter blur+10px 在透明置顶窗的合成开销 (popover.css)。
- [x] 范围外: 不改浮窗视觉/卡片布局/配色 (上一轮已定)；不改 popover_data/stats 后端查询逻辑；不动 hidesOnDeactivate 语义(仍需失活隐藏)；不改其他窗口。
- [x] 约束: macOS 优先(NSWindow.setHidesOnDeactivate)；窗口复用后 hidesOnDeactivate 与 hide 生命周期必须自洽(不能 destroy 后指针失效)；`cargo build` + `cargo clippy` 零 warning；`yarn build` 通过；重定位仍居中于 tray 图标正下方。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [x] 启动时预建 label="popover" 隐藏窗口(不 focused 不显示)，webview 提前 boot。
- [x] tray 左键 Down: 窗口存在→已显则 hide/未显则重定位+show+focus；不再 destroy/rebuild。
- [x] 失焦 Focused(false) + macOS 失活: 改为 hide(非 destroy)，窗口与 NSWindow 指针存活可复用。
- [x] 前端: 复用后再次 show 立即显示上次数据(无空白 loading)，proxy-log 事件仍后台刷新。
- [x] resize/重定位循环精简: 减少每次 show 的 IPC 往返(缓存 scale/center_x)。
- [x] backdrop-filter blur 开销复核并按结论调整(保留或降 radius)。
- [x] `cargo build` + `cargo clippy` 零 warning；`yarn build` 通过；浮窗点即显、失焦即隐、居中正确。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list popover-perf`)
