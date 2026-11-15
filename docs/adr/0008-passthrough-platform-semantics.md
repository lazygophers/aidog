# ADR 0008: 透传平台——est_cost 恒 0、绕过状态机、query 参数鉴权

日期: 2026-10-08
状态: accepted

## 背景

新增平台类型 `passthrough`（透传）：用户只配 base_url + apikey + 认证头形式，请求原样转发（原始 path、body 字节不动、头仅换认证），不识别协议不做转换。usage 出现在响应里就提取，解析不出照常落 proxy_log 记 tokens=0。三期 ask-ui 拍板（共识稿 `.scratch/passthrough-platform/consensus.md`，2026-10-08）。

三个决策与既有机制相反，需要解释为什么。

## 决策

1. **est_cost 恒 0**：透传平台不查 registry 价格，也不走 `PriceSyncSettings` fallback 单价（默认 3.0 $/M）。计费链（ADR 0006 / `billing.rs::calc_est_cost`）对透传平台在 fallback 之前短路返回 0。
2. **绕过状态机**：透传平台不参与失败计数 / auto_disabled / 熔断，永远在线候选。请求只发给分组首选候选，失败直接回给客户端，不切下一个候选（2026-10-19 用户拍板以代码为准，原文「重试链照常」与实现不符）；状态不衰减——上游挂了不自动摘除。
   **2026-10-30 修订**：「永远在线」改为「除客户端白名单外永远在线」。平台配了 `allowed_clients` 时，透传平台同样受约束（聊天路径、body 解析失败、协议子端点、单平台分组均如此）；白名单不使透传平台进入状态机，只在候选过滤阶段排除，其余维度（失败计数 / auto_disabled / 熔断 / 高峰禁用）仍不参与。修订原因：「透传」只免协议解析，不免访问控制（spec `.scratch/平台-客户端白名单/spec.md` 决策 5）。`GET /models` 探测端点仍不受白名单约束。
3. **query 参数鉴权全量兼容**：所有请求（HTTP 与 WS）除 Bearer header 外同时接受 `?api_key=<group_name>` 查询参数定组。 （2026-10-22 起由 [ADR 0009](0009-inbound-key-channels.md) 扩展为多通道 first-match。）

4. **协议无关（2026-10-19 补）**：转发不依赖任何协议解析。body 非 JSON、不符合 path 推断的协议、responses 子端点 / count_tokens / 决策端点、`GET /models` 都原样转发；能解析出 usage 才记 tokens，解析不出照转。

## 理由

1. fallback 单价的语义是「已知协议的模型缺价时兜底」，对完全未知上游套 3.0 $/M 是编造数据；假账比没账更误导统计。
2. 用户拍板（r2-Q4b）：透传平台是兜底语义，自动摘除反而让兜底消失；上游真挂由客户端自己重试。
3. 用户拍板（r3-Q1b）：WS 客户端常无法在握手时加自定义 header，且用户接受 group key 进 URL 的日志暴露面。

## 后果

- 统计页费用不含透传流量（tokens 若解析到仍计入用量统计）。
- 透传平台上游宕机时每个请求都完整尝试一次（可观察 proxy_log 失败行）。
- `?api_key=` 会出现在各类访问日志的 URL 里，等于接受密钥进日志；分组密钥本就是低敏感的自建凭据。
- 代理日志 URL 字段已按现有 retention/脱敏策略处理，不因 query 鉴权额外变化。

## 备选方案

- est_cost 走 fallback：统计连续但数据是编的，弃。
- est_cost 用户可配单价：多一个配置项，第一版不需要，将来可加（复活条件：用户主动要求估算透传流量费用）。
- 状态机照 claude_code 先例（隐藏 tab 但参与）：兜底平台被 auto_disabled 与用户预期不符，弃。复活条件：用户抱怨上游挂了每次都要等超时。
- query 鉴权仅限 WS：暴露面更小，但用户明示全量兼容，弃。

## 修订：透传平台与直通模型不看槽开关（2026-11-10，路由票 01，用户拍板）

1. **透传平台不看内置槽 / 自定义槽开关**（`disabled_slots`），维持本 ADR 的绕过语义：`disabled_slot_reason` 对 `Protocol::Passthrough` 直接返回 `None`（`gateway/router/candidates.rs:1985`），护栏测试 `passthrough_platform_ignores_disabled_slot_adr0008`（`gateway/router/test_candidates.rs:5097`）。
2. **直通模型（`extra.direct_models`）声明优先**，转发请求原名，不判槽开关。
3. 同轮修复：未填值的自定义槽，平台级开关关闭时仍按 `custom_defs_hit` 排除该平台（`ModelNotOnPlatform`）。
4. 落选：透传 / 直通也遵守槽开关。复活条件：用户再报「直通 / 透传平台关槽仍被选中」并要求生效。
5. 直通守卫为 `direct_ids 非空 && 组内平台数 > 1`（`candidates.rs:620-621`），字面单平台组配置 `direct_models` 会被忽略并走原 bypass 链（账本 `.scratch/memory/2026-10-28.md:17`）。复活条件：要求单平台组也报错时改守卫。

来源：`.scratch/memory/2026-11-10.md`「路由票 01」。

## 已裁决不修

| 事项 | 不修理由 | 复活条件 | 裁决日期 | 来源 |
|---|---|---|---|---|
| 透传流式中途断流 / 非流式 body 读失败：attempt 按响应头 2xx 记成功，不改为失败 | 响应头已决定状态，改动面超出本期 | 用户反馈透传失败被画成成功 | 2026-11-04 | `.scratch/memory/2026-11-04.md:244` |
| 透传流与 aux（embedding / rerank）请求不结算 TPM | 账本未记具体理由 | 账本未记，推测为用户要求这些路径计入 TPM | 2026-11-04 | `2026-11-04.md:137` |
| `claude_code` 订阅透传路径（`handle_passthrough`，`passthrough.rs:56`，未设 `strip_inbound_auth`，默认 `false`，`passthrough.rs:76`）不剥 `api-key` / `x-goog-api-key` / `?key=` | 设计上原样转发客户端认证 | 账本未记 | 2026-10-22 | `.scratch/memory/2026-10-22.md:39` |
