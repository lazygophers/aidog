# PRD: 修复上游 gzip 压缩响应未解压（token/成本全 0 + 日志乱码）

## 问题陈述

### 现象
- 请求 id `272c3a3a427045c3b90f5144498405ce`，group `glm-coding-plan-auto`，上游 GLM（platform id=2，`https://open.bigmodel.cn/api/anthropic/v1/messages`），`claude-opus-4-8`→`glm-5.1`，anthropic→anthropic 同协议透传，client=200 / upstream=200，非流式，23s。
- 上游响应头 `content-encoding: gzip`。proxy_log 中 `response_body` 存的是 **gzip 压缩原始字节**（乱码不可读），`input_tokens=0 output_tokens=0 cache_tokens=0 est_cost=$0`。

### 根因链（证据 file:line）
1. **reqwest 未启用解压 feature**：`src-tauri/Cargo.toml:42` `reqwest = { version="0.12", features=["stream","json","socks"] }` —— 无 `gzip`/`brotli`/`deflate`/`zstd`。无对应 feature 时 reqwest 不解压响应体。
2. **客户端 accept-encoding 被透传给上游**：本 case 走主 forward 路径（`same_protocol_passthrough`，非 `/proxy` 裸透传），头底座 `passthrough_convert_headers`（`proxy.rs:2600`）的剔除列表 `STRIPPED_ON_CONVERT_PASSTHROUGH`（`proxy.rs:2578-2595`）**不含 `accept-encoding`** → 客户端 `accept-encoding: gzip, deflate, br, zstd` 原样透传，上游据此 gzip 压缩响应。（裸透传路径 `passthrough_headers`（`proxy.rs:2557`）仅剔 host/content-length，同样不剔 accept-encoding。）
3. **非流式成功路径拿到压缩字节直接当文本**：`proxy.rs:1391-1408` `let body = resp.bytes().await`（gzip 原始字节）→ `String::from_utf8_lossy(&body)`（乱码）→ `extract_usage(&resp_str)`（`proxy.rs:2828`，`serde_json::from_str` 解析乱码失败 → 早返回 `(0,0,0)`）→ token 全 0 → `spawn_estimate` 入参 token 全 0 → est_cost=0（成本依赖 token；见记忆 `pricing-resolve-single-source` / `est-cost-persistence`）→ `log.response_body = resp_str`（乱码入库）。
4. **回客户端 body/头不一致**：`proxy.rs:1422-1423` `user_response_headers` 硬编码 `{"content-type":"application/json"}`，丢弃 content-encoding；但 `user_response_body`（`proxy.rs:1450` 返回的 `body`）仍是 gzip 字节 → 回客户端「gzip 字节 + 声明 json + 无 content-encoding」三者不一致，客户端按 json 解码 gzip 字节会失败。

### 影响范围
- 任何上游回 `content-encoding: gzip/br/deflate/zstd` 的**非流式**响应（GLM anthropic 端点确认 gzip）→ token 统计与成本全 0 + 日志乱码 + 回客户端 body 损坏。
- 同时命中 convert 路径（`proxy.rs:1216` `passthrough_convert_headers`）与裸透传路径（`handle_passthrough` → `passthrough_headers`）——两条转发函数都漏剔 accept-encoding。
- 流式（SSE）路径：上游 SSE 一般以 `text/event-stream` 不压缩，但本任务需在实现中评估确认（见风险节）。

## 目标与非目标

### 目标
- 上游 gzip（及 br/deflate/zstd）压缩的**非流式**响应能被正确解压，得到可读 JSON。
- `extract_usage` 能从解压后 JSON 解析出 token，`est_cost > 0`。
- `proxy_log.response_body` 存可读 JSON（非乱码）。
- 回客户端 body 与声明头一致（发送未压缩 body，不带 content-encoding）。

### 非目标
- 不改估算/定价链路本身（`estimate.rs`/`resolve_price`），token 修正后成本自然非 0。
- 不引入「回客户端重新压缩」（最简一致原则：解压后发明文）。
- 不改前端、不改 DB schema。
- 不为流式路径新增压缩支持（若评估确认上游 SSE 不压缩；若确认压缩则纳入，见验收）。

## 方案对比

reqwest 0.12 解压行为调研结论（引官方 docs.rs，**纠正主会话 prompt 中「手动覆盖 accept-encoding 会禁用自动解压」的假设**）：

- `gzip()`/`brotli()`/`deflate()`/`zstd()` 方法分别由 cargo feature `gzip`/`brotli`/`deflate`/`zstd` 启用。
- **请求侧**：仅当请求头**不含** `Accept-Encoding`（且无 `Range`）时，reqwest 才自动注入对应 `Accept-Encoding`；若已含则尊重不覆盖。
- **响应侧（关键）**：解压由**响应头 `Content-Encoding` 驱动**——"When receiving a response, if its headers contain a `Content-Encoding` value of `gzip`, both `Content-Encoding` and `Content-Length` are removed... The response body is automatically decompressed." 解压**与谁设的 accept-encoding 无关**。故只要启用 feature，即使我们透传了客户端 accept-encoding，reqwest 仍会解压。来源：[docs.rs ClientBuilder::gzip](https://docs.rs/reqwest/0.12/reqwest/struct.ClientBuilder.html#method.gzip)、[DeepWiki reqwest Compression](https://deepwiki.com/seanmonstar/reqwest/4.5-compression-and-encoding)。
- 解压对 `stream` feature 的流式响应同样适用（decoder 包裹 body stream，`resp.bytes()` 与 `bytes_stream()` 都得解压后内容）。

| 维度 | 方案 A（reqwest 原生解压，推荐） | 方案 C（手动解压） |
| --- | --- | --- |
| 改动 | `Cargo.toml:42` reqwest features 加 `gzip,brotli,deflate,zstd`；`resp.bytes()` 直接得明文 | 保留转发；读上游 `content-encoding` 头，按值用 flate2/brotli/zstd crate 手动解压 bytes |
| 是否需停转 accept-encoding | **不需要**（解压由响应 Content-Encoding 驱动，调研已证） | 不需要 |
| 新增依赖 | 仅启用 reqwest 既有 feature（拉入 flate2/brotli/zstd 作 reqwest 内部依赖） | 需直接新增 flate2 + brotli + zstd 三个 crate + 手写分发逻辑 |
| 代码量 | 极小（仅改 Cargo + 无需改 bytes 读取逻辑） | 较多（每编码分支 + 错误处理 + 与流式两处都改） |
| 多编码兼容 | 启用四个 feature 覆盖客户端 accept-encoding 全集（gzip/deflate/br/zstd），上游回任一种都解压 | 需为四种编码各写分支，漏一种即乱码复现 |
| 控制显式度 | reqwest 黑盒（但行为有官方文档背书） | 完全显式可控 |
| 回客户端头处理 | content-encoding 已被 reqwest 移除，天然得明文；按现状发 json 即一致 | 需手动确保不再带 content-encoding |

### 推荐：方案 A
启用四个 feature 一次覆盖客户端 `accept-encoding: gzip, deflate, br, zstd` 全集，改动面最小且行为有官方文档背书。方案 C 仅在「需禁止 reqwest 解压、完全自管」时才有价值，本场景无此需求。

**回客户端 body 一致性处理**（A）：reqwest 解压后 `resp.bytes()` 得明文，`Content-Encoding`/`Content-Length` 已被 reqwest 从 response 头移除；现状 `proxy.rs:1423` 硬编码 `content-type: application/json` 且返回明文 body → 天然一致，无需额外改头（确认不要把上游 content-encoding 透传回客户端）。

## 实现提示（方向，不写完整代码）
1. `Cargo.toml:42`：`features=["stream","json","socks"]` → 追加 `"gzip","brotli","deflate","zstd"`。
2. 非流式路径 `proxy.rs:1391-1408`：`resp.bytes()` 现在已是解压后内容，无需改逻辑；确认 `extract_usage`/`log.response_body`/`replace_model_in_json` 自然得明文。
3. 确认回客户端不透传上游 `content-encoding`（现状已不透传，无需改；勿误加）。
4. 评估流式路径（`proxy.rs:1455+`）：检查上游 SSE 响应是否带 `content-encoding`；启用 feature 后 reqwest 对 `bytes_stream()` 同样解压，预期无需额外改；若评估发现 SSE 也压缩则确认 StreamAggregator 拿到的是解压后块。
5. 新增回归测试：构造一段 gzip 压缩的 anthropic 响应 JSON（含 `usage.input_tokens`/`output_tokens`），断言解压+`extract_usage` 解析出 token>0。

## 验收标准（可机器验证）
- `cd src-tauri && cargo build` 通过。
- `cd src-tauri && cargo clippy` 无 warning（见记忆 `warnings-are-issues`）。
- `cd src-tauri && cargo test` 全绿，含**新增 test**：对一段 gzip 压缩的 anthropic JSON 响应，验证经 reqwest（或等价解压）后 `extract_usage` 返回 token 均 > 0。
- 重发该真实请求（GLM glm-coding-plan-auto，非流式）后：proxy_log 该请求 `input_tokens>0 && output_tokens>0`，`est_cost>0`（用 `aidog-request-inspect` 按 request id 核验）。
- proxy_log `response_body` 为可读 JSON（非乱码），含 `usage` 字段。
- 回客户端响应可被客户端正常解析（无 gzip/json 不一致解码失败）。

## 风险
- **流式路径**：若评估发现上游 SSE 也压缩，需确认 StreamAggregator（`proxy.rs:2649`）旁路拿到的是解压后块，否则流式 token 聚合同样为 0。实现中必须实测确认，禁假设。
- **回客户端 body 一致性**：必须发未压缩 body 且不带 content-encoding；勿在修复中误把上游 content-encoding 透传回客户端（会再次造成不一致）。
- **多上游兼容**：四个 feature 全启用后，所有上游的压缩响应统一解压；理论上无回归（非压缩响应 reqwest 不动）。但需注意启用 feature 会拉入 brotli/zstd 编译依赖，确认编译产物体积可接受（桌面 app，低敏感）。
- **transfer-encoding chunked**：与 content-encoding 正交，reqwest 自处理，无需介入。

## 依赖
无前端改动。仅 `Cargo.toml` + `proxy.rs`（测试）。
