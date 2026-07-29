# Logs/Stats 查询与 IPC 瘦身 — PRD (主入口)

## 目标
- [ ] 消灭 Logs 列表 COUNT(*) 对 7GB proxy_log 表的全扫 —— source_protocol 无索引且 NOT IN 非 sargable，该查询在转发期每 500ms 跑一次，是 SQLite page cache 被灌满的驱动源
- [ ] 砍掉空闲期与转发期的冗余 IPC：Stats 页每次 log 更新全量重拉 group+platform，model 下拉为取一列 distinct 拉 200 行全字段
- [ ] 给 CONNECT 隧道日志路径补上与普通路径对称的 emit gate + 节流，止住托盘主线程最高 5Hz 重绘
- [ ] 消除 PlatformCard 每卡每渲染约 4 次 JSON.parse(extra) 的重复派生计算
- [ ] 本 task 是 sqlite-page-cache-residency 的前置：先掐掉灌 cache 的源头，其 baseline 才建立在正确前提上
## 边界
- 只动查询形态、索引与调用频次，不改 proxy_log 表的既有列语义
- 不动 SQLite cache_size —— 归 sqlite-page-cache-residency，本 task 是其前置
- 不碰 log.db 7GB 体积治理（retention / VACUUM / checkpoint），另开 task
- 不碰前端合成层与动画（归 frontend-compositing-purge）
- Logs / Stats / Platforms 三页的功能、筛选语义、默认 exclude_sources 行为对用户零变化
- 一切量测与压测只用 mock 平台与分组，禁触真实平台；禁写用户真实 log.db（EXPLAIN QUERY PLAN 可 mode=ro 只读跑）
## 验收标准
- [x] EXPLAIN QUERY PLAN 证明 Logs 列表查询不再对 proxy_log 全表扫描
- [x] 转发期（50 路 mock 并发）Logs 页驻留时，proxy_log 上的 COUNT 类全扫查询次数为 0
- [x] Logs 列表首屏加载 p95 相对基线不上升（改分页探测后应下降）
- [x] Stats 页在 proxy-log-updated 事件到达时不再调用 groupDetailApi.list 与 platformApi.list
- [x] Logs model 下拉不再拉 200 行全字段，改为后端 distinct 两列返回
- [x] CONNECT 隧道路径的 proxy-log-updated / tray-refresh emit 具备与普通路径相同的 terminal gate 与节流
- [x] PlatformCard 单次渲染内 JSON.parse(extra) 调用次数为 1
- [x] Logs / Stats 页在相同数据下的展示结果与改动前逐项一致（含分页总数语义变化已在 UI 上正确体现）
- [x] cargo clippy 零 warning、cargo test 全绿
- [x] yarn build 通过、yarn test 全绿、check-i18n 通过
- [x] 全程只用 mock 平台与分组，记录中可核验无真实平台调用、未写入用户真实 log.db
- [x] 清场完成：临时脚本与逐次原始采样已删，只留最终指标对比表
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list logs-query-ipc-slimming`)
