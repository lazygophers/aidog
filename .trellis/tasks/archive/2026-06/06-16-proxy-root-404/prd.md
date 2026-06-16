# 代理根路径 /proxy 404 修复

## 背景
- request_id=b432711e4f084704b661ba7d3a8f3833：path=/proxy, 无 Authorization, status_code=404, response="no matching group"。
- 数据库 12 条请求命中 `/proxy` 精确路径无 auth → 全 404；成功请求进 `/proxy/v1/messages` (200)，确认代理 base URL = `http://127.0.0.1:PORT/proxy`。
- 根因：客户端启动探测命中代理根 URL，走 fallback handle_proxy → resolve_group None → 404，并落 proxy_log 污染统计。

## 目标
客户端根路径探测命中时返回 200 + 身份 JSON，跳过组路由与日志。

## 方案
- proxy.rs Router 新增 `.route("/", get(handle_root))` 与 `.route("/proxy", get(handle_root))`。
- `handle_root` 返回 `(200, Json({"service":"aidog","ok":true}))`，无 state/无 proxy_log/无上游。

## 验证
- cargo clippy 通过（无 warning）。
- cargo test proxy 26 passed。
