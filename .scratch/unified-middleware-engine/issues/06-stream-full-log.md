# 06: 流式日志完整记录（废 [stream] 占位）

**What to build:** 流式响应聚合完整 SSE 文本，流结束后回填 proxy_log 响应体；客户端中断或
上游断流时已聚合的部分照样落库；`[stream]` 占位及其在 retention/终态判定里的特殊分支全删，
新增显式 done 标记列，流结束回填时置位，retention 清理与「原始信息 strip」逻辑改用该列。
中间件不影响日志记录。

**Blocked by:** None (can start immediately)（与 01-05 并行的独立线）

**Status:** done (commit c5f24a6d)

- [x] 完整流：聚合文本回填，done 置位
- [x] 断流：已聚合部分落库，done 语义明确
- [x] `[stream]` 占位不再写入；retention/strip 改用 done 列，测试覆盖
- [x] 非 2xx/重试路径日志行为不回归
- [x] cargo test / clippy 全绿
