# 请求头未记录 — headers 与 body 解耦

## 背景
- request_id=729d085103e246b1bc34888527541117：status=200 成功，但 request_headers / request_body / upstream_request_* 全空。
- 根因（by design）：`proxy/logging.log_user_request=false` → `upsert_log` 内 `strip_user=true` → `ProxyLogColumns::from_log` 把 request_headers 与 request_body 一起清空。
- request_headers 是元数据（Authorization 已脱敏为 `[REDACTED]`），与 request_body（prompt 敏感内容）语义不同，不应捆绑同一开关。

## 目标
解耦 headers 与 body：所有 `*_headers` 字段始终入库 + 不被 user/upstream retention 清理；`*_body` 字段仍受 `log_user_request` / `log_upstream_request` 开关与对应 retention 控制。

## 改动
1. **db.rs `ProxyLogColumns::from_log`** (line ~2005)：
   - `request_headers` / `upstream_request_headers` / `upstream_response_headers` / `user_response_headers` → 始终 clone（去掉 strip 条件）。
   - `request_body` / `upstream_request_body` / `user_response_body` → 保持 strip_user / strip_upstream 条件。
2. **db.rs retention cleanup** (line ~2295, ~2311)：
   - `cleanup_user_request_fields`：SET 去掉 `request_headers` / `user_response_headers`，只清 `request_body` / `user_response_body`；WHERE 条件改 `request_body != '' OR user_response_body != ''`。
   - `cleanup_upstream_request_fields`：SET 去掉 `upstream_request_headers` / `upstream_response_headers`，只清 `upstream_request_body`；WHERE 条件改 `upstream_request_body != ''`。
3. **CLAUDE.md** proxy-log-settings 约定更新：headers = 元数据始终记录，body 受开关控制。

## 验证
- cargo clippy 0 warning。
- cargo test db/proxy 通过（含 from_log 行为若有测试）。
- 手动：log_user_request=false 时新请求 request_headers 非空、request_body 空。
