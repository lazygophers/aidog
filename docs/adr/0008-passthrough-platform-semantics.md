# ADR 0008: 透传平台——est_cost 恒 0、绕过状态机、query 参数鉴权

日期: 2026-10-08
状态: accepted

## 背景

新增平台类型 `passthrough`（透传）：用户只配 base_url + apikey + 认证头形式，请求原样转发（原始 path、body 字节不动、头仅换认证），不识别协议不做转换。usage 出现在响应里就提取，解析不出照常落 proxy_log 记 tokens=0。三期 ask-ui 拍板（共识稿 `.scratch/passthrough-platform/consensus.md`，2026-10-08）。

三个决策与既有机制相反，需要解释为什么。

## 决策

1. **est_cost 恒 0**：透传平台不查 registry 价格，也不走 `PriceSyncSettings` fallback 单价（默认 3.0 $/M）。计费链（ADR 0006 / `billing.rs::calc_est_cost`）对透传平台在 fallback 之前短路返回 0。
2. **绕过状态机**：透传平台不参与失败计数 / auto_disabled / 熔断，永远在线候选。单请求内的重试链照常（本平台失败切下一个候选），但状态不衰减——上游挂了不自动摘除。
3. **query 参数鉴权全量兼容**：所有请求（HTTP 与 WS）除 Bearer header 外同时接受 `?api_key=<group_name>` 查询参数定组。

## 理由

1. fallback 单价的语义是「已知协议的模型缺价时兜底」，对完全未知上游套 3.0 $/M 是编造数据；假账比没账更误导统计。
2. 用户拍板（r2-Q4b）：透传平台是兜底语义，自动摘除反而让兜底消失；上游真挂由重试链兜底。
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
