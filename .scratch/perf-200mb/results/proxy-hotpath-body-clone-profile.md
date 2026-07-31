# 报文深拷贝 profile — request.rs:80 / forward.rs:288（proxy-hotpath-buffers s6）

## 方法

- 临时 `#[ignore]` 微基准（跑完即删，未留在源码）：`serde_json::Value::clone()` / `serde_json::from_value(body.clone())` 在代表性请求体尺寸下的耗时，`cargo test --release`，2000 次迭代取均值，`std::hint::black_box` 防优化消除。
- 请求体尺寸档位：small ~1KB（单轮对话）、medium ~50KB（多轮历史）、large ~500KB（长上下文/近似 base64 附件量级）。

## 调用频次核查（决定是否值得当热点看）

- `request.rs:80`（`gateway/adapter/converter/request.rs::parse_incoming_request`，Anthropic 默认分支）：调用点唯一 `handler.rs:329`，**每请求 1 次**。
- `forward.rs:288`（`same_protocol_passthrough` 分支 `req_value.clone()`）：**每请求 ≤1 次**，且仅同协议透传路径命中。
- 对照组：本 task s4 已修复的 `log.rs` clone 是 **40+ 次/请求**（中间态日志），量级不同，那才是真正需要挪位置的热点；本二处是一次性开销。

## 实测数据

### request.rs:80 — `serde_json::from_value(body.clone())`

| 请求体档位 | 字节数 | `body.clone()` 单独耗时 | `from_value(body.clone())` 全路径耗时 |
|---|---|---|---|
| small ~1KB | 903B | 424ns (0.0004ms) | 1314ns (0.0013ms) |
| medium ~50KB | 40,670B | 3023ns (0.0030ms) | 5068ns (0.0051ms) |
| large ~500KB | 400,670B | 7834ns (0.0078ms) | 16513ns (0.0165ms) |

### forward.rs:288 — `req_value.clone()` + model 字段替换

| 请求体档位 | 字节数 | 耗时 |
|---|---|---|
| small ~1KB | 903B | 473ns (0.0005ms) |
| medium ~50KB | 40,670B | 5261ns (0.0053ms) |
| large ~500KB | 400,670B | 10647ns (0.0106ms) |

## 判定：均非热点，已查，无阻断项

- 两处耗时上限（500KB 大体量请求体）仍 <0.02ms，且**每请求仅发生 1 次**（非循环/非 40+ 次模式）。
- 对比转发热路径其余环节量级：上游 AI API 网络往返 / 流式首 token 时延通常是 数十~数千 ms 级，这两处 clone 开销比该量级低 3-5 个数量级，淹没在噪声里，无法测出可感知影响。
- 红线 1（首 token 时延）无退化风险：本 subtask 未改动这两处代码，结构不变。
- 红线 2（token 计数 / est_cost）不受影响：未改动。

## 结论

- request.rs:80、forward.rs:288 两处报文深拷贝：**已查，无阻断项，不改**。
- 与设计文档「三、报文深拷贝」条目中已修复的 `log.rs` clone（40+ 次/请求，s4 已挪位置）形成对照 —— 频次是决定是否为热点的关键变量，本二处频次（1 次/请求）不构成 CPU 热点，符合 ponytail「先量再改，非热点不硬改」纪律。

## 清场确认

- 临时基准代码（`test_request.rs` 内 `temp_profile_body_clone_cost`、`forward.rs` 内 `temp_profile_passthrough_clone` mod）已 `git checkout --` 还原，未落入正式代码/测试套件。
- `git diff --stat` 核实两文件已无本 subtask 改动残留。
