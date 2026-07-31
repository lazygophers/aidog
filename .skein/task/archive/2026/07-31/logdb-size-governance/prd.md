# log.db 体积治理 — PRD (主入口)

## 目标
- [ ] 查明 ~/.aidog/log.db 涨到 8.7GB 的根因，并按用户拍板的方式把库体积降回可用量级
## 边界
- 只做归因勘察 + 一次性手动清库
- 不改任何代码（用户明确拍板「你可以手动清理一下」，四个代码治理方向一个都没选）
- 不调 retention 参数（实测证明 retention 未失效，调它不解决问题）
- 不动 aidog 源码里的 body 落盘逻辑（根因未修，作为遗留记录在案）
## 验收标准
- [x] 归因结论落盘 findings.md，并明确推翻或证实 task 登记时的「retention 疑失效」假设
- [x] log.db 主库体积降到 1GB 以下
- [x] WAL 已 checkpoint 归零，PRAGMA journal_mode 仍为 wal（未被 VACUUM 改坏）
- [x] 清理前后数据（体积/行数/journal_mode）已落盘为可复核的执行记录
- [x] 清理用的临时脚本已删除，aidog 应用已恢复运行
- [x] 删行带来的历史统计丢失已如实向用户说明（不默默换策略）
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list logdb-size-governance`)
