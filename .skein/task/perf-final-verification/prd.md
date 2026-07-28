# 全进程 200MB 总收口验收 — PRD (主入口)

## 目标
- [ ] 回答唯一的最终问题：全部 8 个性能 task 合入后，aidog 全进程内存总和在默认窗口尺寸下是否 ≤200MB，CPU 是否较优化前显著下降。
- [ ] 7 个改动型 task 各自达标 ≠ 总目标达标 —— 本 task 是唯一的总收口判决点，无它则 spec 交付时缺最终结论。
- [ ] 产出可复现的最终指标集：三场景（空闲前台 / 空闲隐藏 / 50 路并发 mock 流）× 全进程分解（主进程 / WebContent / GPU / Networking / popover）。
- [ ] 产出最终 spec 文档，正面写明达标结论、未达标项的物理归因、以及「用户拉大窗口会超预算」这一物理事实。
- [ ] 逐条回归确认红线 1-4 无破：转发延迟与首 token 时延 / token 计数与费用精度 / 用户体验 / 冷启动速度。
## 边界
- 本 task 只量测与判决，禁改任何生产代码 —— 发现未达标项则登记新 task，不在此修。
- 量测判据只认 footprint 的 phys_footprint 与 heap 的 5KB 块数，禁 ps rss / vmmap（二者漏算 Owned physical footprint (unmapped) (graphics)，[01] 已证）。
- 压测只允许 mock 平台与 mock 分组，禁真实平台（用户硬约束）。
- 达标口径 = 默认窗口尺寸（1026×759）下的全进程总和，非最大化尺寸。
- release 构建量测，每场景独立重启进程、等满 ≥10min 稳态，禁同进程内切场景连采（[03] 栽点）。
- 不做优化建议清单 —— 未达标项只写归因与数字，改法归后续 task。
- 临时脚本与逐次原始采样，本 task 结束即删；results/ 只留最终指标集与 spec 两份。
## 验收标准
- [ ] 8 个前置 task 全部 finish 且已合入 master 后才开始量测（不在半成品上测）。
- [ ] release 构建，默认窗口尺寸 1026×759，空闲前台稳态（≥10min）全进程 phys_footprint 总和已记录并与 200MB 逐项对账。
- [ ] 空闲隐藏场景全进程总和已记录。
- [ ] 50 路并发 mock 流稳态全进程总和已记录（峰值口径）。
- [ ] 三场景 CPU% 已记录（sample 采样），与优化前基线逐场景对比，下降幅度明确。
- [ ] 全进程分解表齐全：主进程 / WebContent / GPU / Networking / popover 各自字节，与 [03] 的预算表（主 30 / WebKit malloc 32 / popover 22 / GPU 18 / Networking 6.7 / 零散 20 / 合成面 71）逐项对账。
- [ ] 红线 1 回归：50 路并发 mock 流下 TTFT 与总延迟，与优化前对比无退化。
- [ ] 红线 2 回归：同一组 mock 请求改前改后逐条比对 token 数与 est_cost 完全一致。
- [ ] 红线 3 回归：逐页人工走查（18 个页面），无视觉缺陷、无交互退化、无布局截断。
- [ ] 红线 4 回归：冷启动到首屏可交互耗时，不慢于优化前。
- [ ] 最终 spec 文档已产出，含达标判决 / 未达标项归因 / 窗口尺寸物理事实说明。
- [ ] 未达标项已逐条登记为新 task（若有），不留口头结论。
- [ ] 临时脚本与原始采样已清理，results/ 仅留最终两份产物。
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list perf-final-verification`)
