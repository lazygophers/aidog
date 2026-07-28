# SQLite page cache 常驻治理 — PRD (主入口)

## 目标
- [x] 把 SQLite page cache 造成的主进程常驻内存增量从当前约 99MB（冷启动 50MB → 稳态 149MB）压到 10MB 以内
- [x] 在不违反红线 3（UI 切页与列表流畅度不得下降）的前提下，用实测数据定出 cache_size 的具体数值，而非拍脑袋
- [x] 产出一条可重复的量测链路，让后续任何 cache/DB 调参都能用同一把尺子验证
## 边界
- 改动面只有一个：给 SQLite 连接补 PRAGMA cache_size（gateway/db/mod.rs 的 3 处写连接 + build_read_pool 的只读连接）
- 不改 READ_POOL_SIZE=8（24 条只读连接维持原样，池大小另开 task，需与 [07] 的 UI IPC 数据合看）
- 不碰 log.db 7GB 体积治理（retention / VACUUM / checkpoint），另开 task
- 不碰前端，不碰合成面/CPU（那些归 perf-200mb 图的其他票）
- 不追求预算表那行『主进程 ≤30MB』——实测冷启动 50MB 已证该数不可达，本 task 只对 cache 增量负责
## 验收标准
- [ ] 量测脚本一条命令跑出完整指标集（三条固定查询各自 p95 / heap 5KB 块数 / 冷启动与稳态 phys_footprint / log.db 与 WAL 体积），可重复且两次连跑偏差 <10%
- [ ] 归因判决实验已跑：极小读档下 heap 5KB 块数显著下降，证实『5KB 块 = SQLite page cache』；若证伪则本 task 停手回 08 票重新归因
- [ ] 基线指标集已采并落盘（SQLite 默认档，log.db >=5GB），含基线最慢 SQL 单列
- [ ] cache_size 读档已参数化（AIDOG_SQLITE_READ_CACHE_KB），可不改代码切档跑二分；写档未引入 env、维持默认
- [ ] 大库曲线表已产出：至少 4 档，每档含三条查询 p95 分列 + 内存增量 + 5KB 块数 + 库体积 + p95 相对基线上升百分比
- [ ] 小库对照组（<100MB）已跑同套档位，给出『定值对小库安全 / 不安全』明确结论
- [ ] 定值落地后：稳态 phys_footprint 减冷启动 phys_footprint <= 10MB
- [ ] 定值落地后：heap 5KB 块数 < 2500（基线约 12436）
- [ ] 定值落地后：三条固定查询 p95 相对基线上升均 <= 10%
- [ ] cargo clippy 零 warning、cargo test 全绿
- [ ] 全程只用 mock 平台与分组，测试记录中可核验无真实平台调用、未触碰用户真实 log.db
- [ ] 清场完成：results/ 只剩基线指标集 + 大库曲线表 + 小库对照表三份最终产物，临时脚本 / 逐次原始采样 / 小库环境副本已删
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list sqlite-page-cache-residency`)
