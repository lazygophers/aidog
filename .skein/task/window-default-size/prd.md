# 窗口默认尺寸与合成面定数 — PRD (主入口)

## 目标
- [x] 删 tauri.conf.json 的 maximized:true，默认窗口回到已声明的 1026x759 —— 按 dev 拟合式 graphics 从约 231MB 降到约 74MB **[动作达成，预期证伪见下]**
- [x] 做一次干净的 release 长稳态窗口-内存曲线量测，定死合成面预算的可信系数（每尺寸独立重启 + 等满增长期，禁同进程内改尺寸）
- [x] 在 spec 中正面写明物理事实：合成面是窗口面积的函数，用户手动拉大窗口就会超 200MB，代码规避不了
- [x] 关闭 map 中窗口尺寸约束这条 fog，给出可交接的结论
## 边界
- [ ] 不加 maxWidth / maxHeight 硬限 —— 用户已拍板：改默认非最大化但不限制上限，用户仍可自由拉大或最大化
- [ ] 只动 tauri.conf.json 的窗口默认配置，不动窗口的其他属性与业务逻辑
- [ ] 不动前端 CSS 与动画（归 frontend-compositing-purge，本 task 依赖其完成以获得干净的量测基线）
- [ ] 量测判据只认 footprint 的 phys_footprint 与 heap 的 5KB 块数，禁 ps rss 与 vmmap
- [ ] 量测在 release 构建下做，dev 数据不可外推
## 验收标准
- [x] tauri.conf.json 不再含 maximized:true，首次启动窗口为 1026x759 非最大化
- [x] release 构建下，默认窗口尺寸的全进程 phys_footprint 总和已量测并记录
- [x] release 长稳态窗口-内存曲线已产出：至少 3 个尺寸，每个独立重启且等满增长期，给出拟合式与常数项
- [x] 拟合结论与 dev 的 7.35e-5 系数 / release 两点的 6.34e-5 系数的差异已有解释或明确标注不可比
- [x] 默认尺寸下是否达标 200MB 有明确结论；若未达标，给出还差多少与缺口归属
- [x] spec 中已写明大窗超预算是物理事实这一条
- [x] 用户在默认尺寸下的可用性未受损（侧栏、设置页、Logs 列表在 1026x759 下布局正常，无内容截断或横向滚动）
- [ ] 清场：**移交 perf-final-verification 收尾时执行**（`.scratch/perf-200mb/assets/results/` 下 22 个采样文件与量测协议仍被其 s1-preflight 引用，过早删除会破坏其冒烟验证）。本 task 已完成清单盘点、零删除，见 `s6-verify.md`
## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list window-default-size`)
