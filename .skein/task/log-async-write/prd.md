# 日志异步写入(不阻塞热路径) — PRD (主入口)

## 目标
- [ ] 所有写入 log 库(proxy_log)的内容当前都在 proxy 热路径 `.await` 阻塞到 DB 落盘完成(upsert_log 全被 await, 49 处 + upsert_connect_log 5 处; call_proxy_log_traced db/mod.rs:740 投递单写后台线程 caller 等完成)。目标: 热路径改非阻塞 fire-and-forget, 请求不再等日志落库。

## 边界
- [ ] 范围内: 方案 B —— 单 writer task + 有界 mpsc channel。ProxyState 加 `tx`; 热路径调用点从 `upsert_log(...).await` 改为 `enqueue_log(...)`(同步 send 瞬时返回); writer task 单消费者 FIFO 串行落库。snapshot diff 逻辑(log_snapshots 读改写, log.rs:130→137/145)整体移入 writer task 消费侧串行做, 消除竞态。改动集中 `gateway/proxy/log.rs` + ProxyState, 40+ 调用点仅换调用形态不改语义。
  - **背压策略(已定)**: 队列满时中间态(status=0 / response_body=="[stream]" 占位)可丢弃(不影响最终数据/统计/cost/emit); 终态(status_code!=0)必落 —— 满时终态阻塞 send 等消费或走保留通道。`agg_mark_first` 去重 + upsert_stats_agg + cost 计算 + 前端 emit 均在 writer 侧执行以维持「每请求 +1 一次」语义。
  - `remove_log_snapshot`(log.rs:153 / stream.rs:224) 必须与写在同一 writer 串行序内(否则清早致后续 upsert prev=None 走 INSERT 撞主键)。
  - CONNECT 路径 upsert_connect_log(一次性终态 INSERT 无中间节点不碰 snapshot)可走同一 writer, 最易改。
- [ ] 范围外: 不改 call_proxy_log_traced chokepoint 本身(已是独立 proxy_log 写槽 self.4); 不动 stream.rs:222 与 RequestLogGuard(handler.rs:60)已有的 spawn(每请求仅 spawn 一次终态, 安全); 不动 retention 清理逻辑(仅需确认不与 writer 争写连接, 见约束)。
- [ ] 约束(改造前必处理):
  - 保序: 同 request_id INSERT 必先于 UPDATE, UPDATE 间按序(prev 基线) —— 单 writer 消费者恢复串行。
  - 终态不丢: 背压满时终态必须入队(区分终中态)。
  - 关机 drain: 进程退出时 mpsc 未落盘终态需 flush(shutdown drain)。
  - retention 交互: 确认 retention 清理与 writer 是否共用 log.db 写连接(self.4), 避免两写者竞争。**需要 exec 阶段定位 retention 实现文件确认**(scope 未在 gateway/ 内找到)。

## 验收标准
- [ ] ProxyState 加 mpsc tx + writer task; 热路径 upsert_log/upsert_connect_log 调用改非阻塞 enqueue; 保序由单 writer 保证; 背压中间态丢/终态留; snapshot 生命周期在 writer 串行序内; shutdown drain; retention 写连接确认无竞争; `cargo build` + `cargo clippy`(warning 清) + `cargo test`(db/proxy 相关)全过; 手测代理请求日志正常落库(Logs 页可见, 统计/cost 正确)。

## 索引
- [ ] 详细设计: [design.md](design.md) (writer task 结构 + mpsc 消息类型 + 背压分支)
- [ ] 任务/子任务/调度: task.json (`skein subtask list log-async-write`)
