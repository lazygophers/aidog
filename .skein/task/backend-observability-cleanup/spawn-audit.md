# spawn_traced 迁移审计（s3b 重做）

全仓 `tokio::spawn(` 共 33 处（`grep -rn 'tokio::spawn(' src-tauri/crates --include='*.rs'`）。
判定基准：有请求/命令上下文的任务 → 替换为 `spawn_traced`；启动期/无上下文常驻任务、或纯测试代码 → 有意保留。

| file:line | 已替换 / 有意保留 | 理由 |
|---|---|---|
| logging.rs:329 | 有意保留 | `spawn_traced` 自身实现内部调用，是被包装的原语，不能自己包自己 |
| gateway/http_client.rs:211 | 有意保留 | `#[cfg(test)]` 单测（`build_http_client_disables_env_proxy_when_no_db_proxy`），stub proxy accept 循环，无请求上下文 |
| gateway/http_client.rs:225 | 有意保留 | 同上单测内 stub 上游 axum server，测试脚手架 |
| gateway/manual_budget.rs:444 | 有意保留 | `#[cfg(test)]` 单测（`zero_budget_shortcircuits_write_conn`），模拟长占写连接的辅助 task，无请求上下文 |
| gateway/proxy/log.rs:363 | 有意保留 | 生产代码 `spawn_estimate`，但已显式 `.instrument(span)` 手动传播父 span；`logging.rs:318-319` 文档明确该类调用点保持原状（双重 instrument 会丢父 span 关联），属既有约定的例外 |
| gateway/proxy/test_e2e_mitm.rs:84 | 有意保留 | `#[cfg(test)]` 模块，e2e mitm 测试起本地 axum server |
| gateway/proxy/test_e2e_mitm.rs:158 | 有意保留 | 同上，测试辅助 task |
| gateway/proxy/test_e2e_mitm.rs:179 | 有意保留 | 同上，测试内 conn.await 转发 |
| gateway/proxy/test_e2e_mitm.rs:291 | 有意保留 | 同上，测试辅助 task |
| gateway/proxy/test_e2e_mitm.rs:320 | 有意保留 | 同上，测试内 conn.await 转发 |
| gateway/proxy/test_connect.rs:109 | 有意保留 | `#[cfg(test)]` CONNECT 隧道测试，起测试 server |
| gateway/proxy/test_connect.rs:154 | 有意保留 | 同上，测试辅助 task |
| gateway/proxy/test_connect.rs:156 | 有意保留 | 同上，测试内嵌套 spawn |
| gateway/proxy/test_connect.rs:182 | 有意保留 | 同上 |
| gateway/proxy/test_connect.rs:221 | 有意保留 | 同上 |
| gateway/proxy/test_connect.rs:224 | 有意保留 | 同上，测试内嵌套 spawn |
| gateway/proxy/test_connect.rs:260 | 有意保留 | 同上 |
| gateway/proxy/test_connect.rs:356 | 有意保留 | 同上 |
| gateway/proxy/test_connect.rs:358 | 有意保留 | 同上，测试内嵌套 spawn |
| gateway/proxy/test_connect.rs:382 | 有意保留 | 同上 |
| gateway/proxy/test_connect.rs:435 | 有意保留 | 同上，测试起 axum server |
| gateway/proxy/test_connect.rs:600 | 有意保留 | 同上 |
| gateway/proxy/test_connect.rs:612 | 有意保留 | 同上 |
| gateway/proxy/test_integration.rs:25 | 有意保留 | `#[cfg(test)]` 集成测试辅助 task |
| gateway/proxy/test_integration.rs:37 | 有意保留 | 同上 |
| gateway/proxy/test_integration.rs:1142 | 有意保留 | 同上，测试起 axum server |
| gateway/proxy/test_integration.rs:1151 | 有意保留 | 同上 |
| gateway/proxy/test_integration.rs:1254 | 有意保留 | 同上 |
| gateway/mitm/tls.rs:269 | 有意保留 | `#[cfg(test)]` 单测（`tls_handshake`），in-memory duplex server 端握手 task |
| gateway/mitm/tls.rs:276 | 有意保留 | 同上，client 端握手 task |
| gateway/db/test_rw_pool.rs:128 | 有意保留 | `#[cfg(test)]` 模块（db/mod.rs:1163），读写池并发测试 |
| gateway/db/test_rw_pool.rs:139 | 有意保留 | 同上，reader task 批量 spawn |
| gateway/quota/test_http.rs:17 | 有意保留 | `#[cfg(test)]` 模块（quota/http.rs:230），mock http server |

## 结论

33 处全部核实：31 处为 `#[cfg(test)]` 测试脚手架（本地 mock server / 并发辅助 task，无真实请求链路，替换无意义且会把 trace_id 噪声灌进测试日志），1 处（`logging.rs:329`）是 `spawn_traced` 封装本体的实现行、非调用点，1 处（`proxy/log.rs:363` `spawn_estimate`）虽是生产请求路径但已有手动 `.instrument(span)` 显式传播父 span，属 `logging.rs:318-319` 文档记载的既定例外，重复包裹 `spawn_traced` 会导致双重 instrument、父子链路断裂，故不改。

本轮无可等价替换的裸 `tokio::spawn` 调用点——不存在应替换而遗漏的生产请求/命令路径。

替换数：0　保留数：33
