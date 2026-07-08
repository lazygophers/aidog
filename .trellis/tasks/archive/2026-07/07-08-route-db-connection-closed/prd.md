# route 偶发 400 "ConnectionClosed" — DB 连接修复

## Goal

代理偶发返回 400，`proxy_log.response_body = "route error: ConnectionClosed"`。用户拿到的 request_id=abe076efa8c34e6fb96955faad947c56 即此故障。根因：`select_candidates_ctx` 首个 DB 调用 `db::get_group_platforms(db, group.id).await?`（`router/candidates.rs:57`）直传错误，tokio_rusqlite Connection 偶发 `ConnectionClosed` 时无重试/重连 → 整条 route 失败 → handler.rs:384 落 400。

## 症状（数据支撑）

free 组近期请求分布（DB 查）：
- 200: 805 次（98.9%）
- **400: 8 次（≈1%，max_dur 2318ms）** ← 本 bug
- 429: 66 次（限流，无关）
- 499: 19 次（客户端取消，无关）

**偶发**，非持续。特征：
- `status_code=400`, `upstream_status_code=0`（没到上游）
- `target_protocol=""`, `actual_model=""`, `upstream_request_url=""`（route 阶段就失败）
- `blocked_by=""`, `blocked_reason=""`（非 peak_disabled / 非 NoCandidate，走 handler.rs:384 通用 route fail 分支）
- `duration_ms` 16~2318ms 不等

样本 request_id：`abe076efa8c34e6fb96955faad947c56`（/proxy/v1/messages?beta=true, body 89KB, is_stream=1, group=free, model=claude-opus-4-8）。body 大小是巧合，非根因。

## Root Cause（初步定位，agent 需验证 + 深挖）

1. `Db(pub AsyncConnection, Arc<DbCache>, ReadPoolHandle)`（`db/mod.rs:174`）—— 主 Connection 是单个 tokio_rusqlite Connection（MPSC channel）
2. tokio_rusqlite Connection 在以下情况返 `ConnectionClosed`：
   - closure 内 panic（tokio_rusqlite catch 后关 channel）
   - channel 被 drop（Db 被 drop / 应用关闭）
   - 内部 task 结束
3. `call_traced`（`db/mod.rs:372`）调 `self.0.call(...)`，**无 ConnectionClosed 检测 / 无重试 / 无重连**
4. `get_group_platforms`（`db/group_platform.rs:188`）`?` 直传 ConnectionClosed
5. `select_candidates_ctx`（`router/candidates.rs:57`）`?` 直传
6. `handler.rs:367-391` route fail → 400

## Requirements

1. DB 调用遇到 `ConnectionClosed` 时**自动重连重试**（而非直传 400）
2. 或在 handler 层降级（重试一次 route）
3. 修复后偶发 400 消失（ConnectionClosed 不再直达用户）
4. 不破坏现有 DB 调用语义（其他错误仍正常传播）
5. 加诊断日志（ConnectionClosed 发生时 warn + 计数，便于验证修复效果）

## Decision（待 agent 深挖后定）

**候选方案**（agent 评估选一）：
- **A. Db 层重连**：`call_traced` / `call_read_traced` 检测 ConnectionClosed → 重建 tokio_rusqlite Connection → 重试一次。影响面：所有 DB 调用统一受益。
- **B. handler 层重试**：handler.rs:367 route fail 时若错误含 ConnectionClosed → 重试 select_candidates_ctx 一次。影响面：仅 route 路径，其他 DB 调用不受益。
- **C. 找根因根治**：定位 ConnectionClosed 为何偶发（panic in closure? 连接被某处误关?）→ 从源头消除。

倾向 A（统一兜底）+ C（找源头）并行：A 兜底防用户可见，C 消除根因。

## Acceptance Criteria

- [ ] ConnectionClosed 时 DB 调用自动重试/重连，不返 400 给客户端
- [ ] 重试有上限（1~2 次），避免死循环
- [ ] 重试失败仍走原错误路径（不掩盖其他 DB 错误）
- [ ] 诊断日志可观测（grep "ConnectionClosed" / "db reconnect" 能定位）
- [ ] `cargo clippy` 0 warning
- [ ] `cargo test` 通过（db / router / proxy 现有测试）
- [ ] 新增单测：模拟 ConnectionClosed → 验证重试成功（如可 mock）

## Out of Scope

- 不改路由逻辑（select_candidates_ctx 的候选选择算法）
- 不改 peak_disabled / NoCandidate 等其他 route fail 分支
- 不改前端（400 错误展示是另一回事）
- 不优化连接池（ReadPoolHandle 是读池，主 Connection 是写/路由用，本 task 只修 ConnectionClosed 兜底）

## Technical Notes

- 关键文件：
  - `src-tauri/src/gateway/db/mod.rs:174`（Db 结构）/ `:372`（call_traced）/ `:424`（call_read_traced）
  - `src-tauri/src/gateway/router/candidates.rs:57`（首个 DB call）
  - `src-tauri/src/gateway/proxy/handler.rs:367-391`（route fail → 400）
- tokio_rusqlite Connection API：`.call(closure)` 返 `Result<R, tokio_rusqlite::Error>`，Error 有 `ConnectionClosed` 变体
- 测试参考：`db/test_schema.rs:70` 注释提到 ConnectionClosed 是 tokio_rusqlite 已知模式
- 验证数据源：`~/.aidog/aidog.db` `proxy_log` 表（`response_body LIKE '%ConnectionClosed%'` 可查历史频率）
