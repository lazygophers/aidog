# PRD: 非流式回客户端透传上游响应头（选择性剔除压缩/长度/逐跳头）

- task id: `passthrough-upstream-resp-headers`
- base branch: `next`
- 范围: 仅 `src-tauri/src/gateway/proxy.rs`（非流式 2xx 成功路径 + 流式 SSE 两条路径）+ 一个头筛选辅助函数及其单元测试
- 主交付: **非流式 + 流式 SSE 均透传上游响应头**（用户 2026-06-17 决策纳入流式，见 §6）

## 1. 问题陈述

### 现象
用户希望上游真实响应头（如 `date` / `x-log-id` / `x-process-time` / `vary` / `set-cookie` / `strict-transport-security` 等）能自动透传回客户端。当前代理只回固定 `content-type: application/json`，丢弃上游所有其他响应头。源自请求 `272c3a3a427045c3b90f5144498405ce`（GLM 非流式）。

### 当前实际行为（证据 file:line）
非流式 2xx 成功路径 `proxy.rs:1391-1452`：
- `proxy.rs:1447-1452` 实际返回 `(StatusCode::OK, [(CONTENT_TYPE, "application/json")], body).into_response()` —— **只带 status + 写死的 content-type + body，未附任何上游响应头**。
- `proxy.rs:1423` 把 `log.user_response_headers` 写死为 `{"content-type":"application/json"}`。

上游响应头**已被捕获**但仅用于日志：`proxy.rs:1268-1279` 从 `resp.headers()` 构造 JSON 存入 `log.upstream_response_headers`，**未用于构造回客户端的 Response**。

### 🔴 关键澄清：日志字段 ≠ 实发 HTTP 头（实现 agent 必须区分）
本仓库存在两个**不同**的概念，极易混淆：

| 概念 | 含义 | 位置 |
| --- | --- | --- |
| `log.user_response_headers` | **写入 proxy_log 数据库的日志字符串**，仅供事后审计/排查展示 | `proxy.rs:1423` 等多处 |
| 实际 axum Response 的 headers | **真正通过 HTTP 发回客户端的头** | `proxy.rs:1447-1452` 的 `.into_response()` 结果 |

二者当前各自独立硬编码，互不影响。**仅修改 `log.user_response_headers`（日志行）不会改变客户端收到的头，等于没做需求。** 真正的修复 = **构造一个带选择性上游头的 axum `Response`**（用 `Response::builder` 或往 response 的 `headers_mut()` 插入筛选后的上游头），并**同步**把 `log.user_response_headers` 更新为「实际所发的头」以保持日志与现实一致。

## 2. 目标

- G1: 非流式 2xx 成功路径（`proxy.rs:1391-1452`）把上游响应头（经黑名单剔除后）透传到回客户端的 axum Response 真实 HTTP 头中。
- G2: 同步更新 `log.user_response_headers`（`proxy.rs:1423`）使其反映**实际发回客户端的头集合**，不再写死 `content-type`。
- G3: 提供一个**纯函数**完成「上游 headers → 筛选后 headers」的映射（剔除黑名单、保留业务头），便于单元测试覆盖。
- G4: `cargo build` / `cargo clippy`（warning 必须清）/ `cargo test` 全绿，新增 test 覆盖筛选函数。

## 3. 非目标

- 不改请求头（仅「上游→客户端」响应头单向透传，不涉及客户端→上游请求头）。
- 不改非流式**错误**路径（`proxy.rs:1281-1364`，非 2xx）的回包头 —— 错误体已有 `error_rule` override 语义，本 task 不动。
- 不改 middleware blocked / count_tokens 等辅助回包路径（`proxy.rs:585-594`、`1147-1165`）。
- 不引入用户可配置的头白/黑名单 UI（固定策略，后续需要再开 task）。
- （流式已纳入主交付，见 §6 —— 此前的「流式不在主交付」非目标已作废。）

## 4. 透传策略：白名单透传 + 黑名单剔除

采用「**默认透传上游全部响应头，按固定黑名单剔除**」策略（而非维护正向白名单），保证 `x-log-id` / `x-process-time` 等未来新增的上游自定义头也能自动透传。

### 4.1 必剔黑名单（MUST 剔除）

| 头 | 剔除原因 |
| --- | --- |
| `content-encoding` | 🔴 **核心原因**：上一 task `06-17-upstream-gzip-decompress`（已合并）启用了 reqwest gzip/brotli/deflate/zstd 自动解压，回客户端 body 是**解压后明文**。若透传 `content-encoding: gzip`，客户端会对明文再次 gunzip 失败 —— 直接反向重现刚修复的 bug。reqwest 解压后通常已从 `resp.headers()` 移除该头，但**仍须显式剔除做防御**（防御 reqwest 未移除 / 部分编码残留的情况）。 |
| `content-length` | 解压后 body 字节数已变，原 `content-length` 失真。须由 axum/hyper 按实际 body 重新生成。 |
| `transfer-encoding` | 逐跳头（hop-by-hop，RFC 7230 §6.1）。`chunked` 等传输编码由本代理与客户端之间的连接独立协商，不能照搬上游值；须由 server 框架按实际连接生成。 |

### 4.2 应剔逐跳头（SHOULD 剔除，依据 RFC 7230 §6.1）

RFC 7230 §6.1 定义的 hop-by-hop 头在转发时**不得**透传给下一跳。建议一并剔除：

- `connection`、`keep-alive`：连接级控制头，由本代理↔客户端连接独立管理；透传上游的 `connection` 值可能错误声明本连接的行为。**建议剔除**。
- `proxy-authenticate`、`proxy-authorization`、`te`、`trailer`、`upgrade`：同属 hop-by-hop，实际上游响应中罕见，但为合规一并纳入剔除集。

> 依据：RFC 7230 §6.1 "Connection" —— hop-by-hop 头「are intended only for a single transport-level connection, and MUST NOT be communicated by proxies over further connections」。

### 4.3 透传保留（典型业务头，全部保留）

`date` / `x-log-id` / `x-process-time` / `vary` / `set-cookie` / `content-type` / `strict-transport-security` / `etag` / `x-request-id` 等以及任何不在黑名单中的上游头。

> 注：`content-type` 透传上游真实值（而非写死 `application/json`），因上游可能返回 `application/json; charset=utf-8` 等带参数形式；若上游缺失 `content-type`，回退默认 `application/json`。

### 4.4 `set-cookie` 安全性评估
代理场景下，上游下发的 `set-cookie` 透传给客户端**一般可接受**：cookie 属于上游 API 与其客户端之间的会话状态，aidog 作为透明代理透传不改变信任边界。本 task 默认透传（不剔除）。**注记风险**：若未来 aidog 自身在同 host 下设置 cookie，需评估命名冲突；当前无此场景，暂不处理。

### 4.5 头名大小写与多值
- HTTP 头名大小写不敏感；黑名单匹配须**小写归一化**比较（`HeaderName` 本身即小写存储，用 `as_str()` 比对小写常量即可）。
- 多值头（如多个 `set-cookie`）须逐个 append 保留，**禁用** map 覆盖语义丢值。

## 5. 实现要点

1. **新增纯函数**（建议签名）：`fn filter_upstream_resp_headers(src: &reqwest::header::HeaderMap) -> Vec<(http::HeaderName, http::HeaderValue)>` 或直接产出 axum `HeaderMap`。逐项遍历 `src`，跳过 §4.1+§4.2 黑名单（小写常量集合），其余转为 axum header 类型收集；多值头逐个保留。无法转换的非法 value 跳过（不 panic）。
2. **构造带头 Response**（核心，非仅改日志）：在 `proxy.rs:1447-1452`，把当前 `[(CONTENT_TYPE, "application/json")]` 数组替换为基于筛选结果构造的 Response。推荐先 `let mut resp = (StatusCode::OK, body).into_response();` 再 `resp.headers_mut().extend(filtered)`；若 filtered 不含 `content-type` 则补默认 `application/json`。或用 `Response::builder()` 等价实现。
3. **同步日志字段**：把 `proxy.rs:1423` 的写死值替换为「实际所发头集合」序列化为 JSON（与 `upstream_response_headers` 同格式：`{header_name: value}`，多值可合并或保留首值，与现有 `log.*_headers` 格式约定一致）。**日志值必须 = 实发头**，不得再各写各的。
4. **不改 body 处理链**：model remap（`1404-1408`）、middleware outbound（`1413-1421`）、usage 提取保持不变。头处理在 body 最终确定后、return 前进行。

## 6. 流式路径（纳入主交付 —— 用户 2026-06-17 决策）

流式 SSE 有两条返回路径，均需叠加上游头透传：
- 转换 SSE：`proxy.rs:1618-1638`，当前硬编码 `text/event-stream` + `cache-control: no-cache` + `connection: keep-alive`，不带上游头。
- 同协议 passthrough relay：`proxy.rs:1768-1780`，同样硬编码三头。

**流式透传策略**（在非流式黑名单基础上 **额外** 剔除 SSE 自管头，防止上游头覆盖 SSE 语义）：
1. 保留 SSE 三个自管头：`content-type: text/event-stream` / `cache-control: no-cache` / `connection: keep-alive`（不被上游值覆盖）。
2. 叠加筛选后的上游头，但在 §4.1+§4.2 黑名单之外 **再额外剔除** `content-type` / `cache-control` / `connection`（这三者归 SSE 自管，禁用上游值覆盖）。
3. 即流式筛选函数 = 非流式筛选结果 ∖ {content-type, cache-control, connection}，再与 SSE 三自管头合并。
4. 透传价值头（`x-log-id` / `x-process-time` / `date` / `set-cookie` / `vary` 等）在流式下同样透传。

**实现建议**：复用同一纯函数 `filter_upstream_resp_headers`，对流式路径在其输出上再过一层 SSE 额外黑名单（或函数加一个 `is_stream` 参数控制额外剔除集）。两条流式路径（1618 / 1768）都改。`upstream_response_headers` 在流式路径的捕获点：`proxy.rs:1902` 附近（与非流式 1278 对应）。

## 7. 验收标准（可机器验证）

- AC1: `cd src-tauri && cargo build` 成功。
- AC2: `cd src-tauri && cargo clippy` 零 warning。
- AC3: `cd src-tauri && cargo test` 全绿。
- AC4: **新增单元测试**覆盖筛选函数 `filter_upstream_resp_headers`，断言：
  - 输入含 `content-encoding: gzip` / `content-length: 123` / `transfer-encoding: chunked` / `connection: keep-alive` → 输出**不含**这些头。
  - 输入含 `date` / `x-log-id` / `x-process-time` / `vary` / `set-cookie` / `content-type` → 输出**保留**这些头且值不变。
  - 多个 `set-cookie` → 输出保留全部（不丢值）。
  - 头名大小写混合（如 `Content-Encoding`）→ 仍被正确剔除。
- AC5: 非流式成功路径回客户端的 axum Response **实际 HTTP 头**包含筛选后的上游头（不再只有 content-type）。可通过：(a) 针对该路径的集成/单元测试断言 response.headers()；或 (b) 实跑代理对 GLM 非流式请求，用 `aidog-request-inspect <req_id>` 或客户端抓包确认 `date` / `x-log-id` 等已透传、`content-encoding` 未透传。
- AC6: `log.user_response_headers`（proxy_log）值等于实际发回客户端的头集合（不再写死 `content-type`）。
- AC7（流式）: 两条流式路径（`proxy.rs:1618` / `1768`）回客户端 Response 头 = SSE 三自管头（`text/event-stream` / `cache-control: no-cache` / `connection: keep-alive`）+ 叠加筛选上游头（额外剔 content-type/cache-control/connection），即 SSE 语义头不被上游覆盖、透传头（x-log-id/date/set-cookie 等）出现。新增/扩展 test 覆盖流式额外黑名单（断言 content-type/cache-control/connection 取 SSE 自管值，x-log-id 来自上游）。

## 8. 影响面与风险

- **R1（高，已规避）`content-encoding` 误透传破坏客户端解码**：若黑名单遗漏 `content-encoding`，客户端对解压后明文再次解码失败 —— 反向重现 `06-17-upstream-gzip-decompress` 修的 bug。§4.1 显式剔除 + AC4 测试兜底。
- **R2（中）`content-length` 失真**：解压后长度变化，必须剔除让框架重算，否则客户端按错误长度截断/挂起。§4.1 覆盖。
- **R3（低）`set-cookie` 透传**：见 §4.4，代理场景一般可接受，默认透传并注记。
- **R4（低）非法 header value 转换失败**：reqwest header value 可能含 axum 不接受的字节；实现须跳过而非 panic（§5.1）。
- **R5（低）流式未覆盖**：本 task 不动流式，流式仍只回固定三头 —— 与现状一致，无回归。
- **R6（低）日志格式一致性**：`user_response_headers` 新值须与既有 `upstream_response_headers` JSON 格式约定一致，避免前端日志展示解析差异。
