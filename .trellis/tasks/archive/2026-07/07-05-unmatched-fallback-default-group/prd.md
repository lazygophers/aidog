# PRD — 未匹配分组 fallback 默认分组记录统计（不报 404）

## 背景
`curl -x http://127.0.0.1:9892/proxy https://www.baidu.com`（baidu 在 MITM 白名单）：
CONNECT 200 → MITM 解密 TLS 握手成功（AirDog MITM CA 签证书）→ 明文 `GET /` 灌 handle_proxy →
`resolve_group` 无 Authorization → 返 404 `NoMatchingGroup`（handler.rs:232）。

当前无 Authorization（或错 token）一律 404，对 MITM 解密的**普通 HTTPS 浏览流量**不合理 ——
用户配代理浏览网页（非 API 调用），应直通原 host，不应因无 group token 被拒。

## 根因
- `handler.rs:218` `resolve_group` 返 `None` → `:228-244` 直接返 404，无 fallback 分支。
- MITM 解密流量（CONNECT target = baidu.com）与 API 流量（直连代理自身 host）都进 handle_proxy，
  当前不区分，一律要求 group token。

## 目标（用户决策已定）
MITM 解密的**非 API**流量未匹配分组时：
1. **不报 404**，直通原 host 转发（透明，不选平台、不计费）。
2. 落 `proxy_log` 记**虚拟统计桶**：`group_id=0` + `group_name="未匹配"`（不入 `groups` 表）。
3. 前端统计单独展示「未匹配」桶。

API 流量（`/v1/messages` 等）未匹配 token **仍 404**（配错 token 应报错，不能静默旁路计费）。

## 判定逻辑（核心，exec 实现）
`resolve_group` 返 `None` 后，进 fallback 判定：
1. **MITM 解密识别**：请求 `Host` header ≠ 代理自身监听 host（127.0.0.1:9892 / localhost:port）
   → 来自 CONNECT target（MITM 解密或 blind_relay 直连后的明文灌入）。
   - 代理自身 host 直连（API 客户端调 `http://127.0.0.1:9892/v1/messages`）→ Host = 代理自身 → 不走 fallback。
2. **非 API path 判定**：path 命中 API endpoint 清单（`/v1/messages`, `/v1/chat/completions`,
   `/v1/responses`, `/v1/embeddings`, `/v1/models` 等及 responses/count_tokens 子端点）→ 仍 404。
3. 两条件都满足（MITM 解密 + 非 API）→ **直通原 host**：用 reqwest 构造
   `https://{orig_host}{orig_path}?{query}`（保留原 method/headers/body），转发响应回客户端。
4. `proxy_log` 落 `group_id=0` + `group_name="未匹配"` + `platform_id=0` + 不计费（cost=0），
   保留 url/status/duration/model 等元数据用于统计。

## 产出

### D1 — handler.rs fallback 分支
`resolve_group` None 分支（handler.rs:220-244）改造：不再直接 404，先调 `should_fallback_passthrough`
判定（见 D2），命中 → 调 `forward_passthrough_to_orig_host`（见 D3），落 D4 统计；未命中 → 保留原 404 逻辑。

### D2 — is_api_endpoint + should_fallback_passthrough（endpoint.rs 或 handler.rs）
- `is_api_endpoint(path: &str) -> bool`：API 路径清单（复用 `is_models_endpoint` /
  `is_responses_subendpoint` 已覆盖的 + 补 `/v1/messages` / `/v1/chat/completions` /
  `/v1/embeddings` 等主路径）。
- `should_fallback_passthrough(host: &str, path: &str, proxy_listen_host) -> bool`：
  `host != proxy_listen_host && !is_api_endpoint(path)`。

### D3 — forward_passthrough_to_orig_host（forward.rs 或新 mod）
reqwest 构造 `https://{host}{path}`（method/headers/body 透传，剥 proxy-only headers 如
`Proxy-Authorization`/`Proxy-Connection`），发请求，流式回传响应 body + status + headers。
超时/错误 → 返 502 + 落 proxy_log。

### D4 — proxy_log 虚拟桶
`log.group_id = 0`（proxy_log.group_id 列允许 0/NULL）；`log.group_name = "未匹配"`；
`log.platform_id = 0`；`log.cost = 0`（不计费）；其他元数据照常。统计查询（见 D5）聚合 group_name。

### D5 — 前端统计展示
- `get_group_usage_stats`（db.rs）当前按 `proxy_log.group_name` 聚合，group_name="未匹配" 的行自动成桶。
  确认查询不过滤 group_id=0（test_usage_stats.rs:178 注释提到「空 group_key 的日志：批量结果中不应出现」
  → 需复核该过滤是否会吞掉虚拟桶；若吞，调整 WHERE 保留 group_name="未匹配"）。
- Groups 页：虚拟桶单独展示（只读卡片，标记「未匹配」，灰色或徽标区分真实分组；无平台/余额字段）。
- Stats 页趋势：含 group_id=0 行（按当前 Stats 聚合逻辑自然包含，复核）。

## 验证
- [ ] `cargo test gateway::proxy`（fallback 判定 + 直通转发单测；mock 上游返回 200，断言 proxy_log group_id=0/group_name="未匹配"）
- [ ] `cargo test gateway::db::test_usage_stats`（虚拟桶出现在统计结果，不被过滤吞掉）
- [ ] `cargo clippy` 0 warning
- [ ] 实测：用户 `yarn tauri dev` 后 `curl -x ... https://www.baidu.com` 不再 404，能拿到 baidu 响应；
  代理日志页 / Groups 页统计可见「未匹配」桶含该请求。
- [ ] 实测回归：`curl -x ... -H "Authorization: Bearer wrong-token" https://127.0.0.1:9892/v1/messages`
  仍 404（API 流量未匹配不旁路）。

## 非目标
- ❌ 改 MITM 白名单逻辑 / blind_relay（P1 隧道非白名单 host 已盲转，不进 handle_proxy）
- ❌ 改 groups 表结构 / 新增真实分组
- ❌ API 流量未匹配 fallback（仅非 API）
- ❌ 直通流量计费（虚拟桶 cost=0，仅统计请求数/流量）

## grill 自审 trace
- 轴 A 目标 ✓ MITM 非 API 未匹配 → 直通 + 虚拟桶统计，封闭（API 仍 404）
- 轴 B 产出 ✓ D1-D5 五交付，各附验收（test/实测/前端可见）
- 轴 C 验证 ✓ cargo test + curl baidu 实测 + API 回归 404，可执行断言
- 轴 D 资源 ✓ handler.rs / endpoint.rs / forward.rs / db.rs / Groups.tsx + Stats.tsx，单 task 串行
- 轴 E 依赖 ✓ 单 task 无并行冲突
- 轴 F 失败 ✓ is_api_endpoint 漏判 → 误直通 API（防御：清单复用既有 is_models/responses + 主路径）；
  MITM 识别误判 → 代理自身 API 流量被直通（防御：host ≠ 代理自身 host 硬条件）
- 轴 G 检查点 ✓ 用户已 3 问决策（转发/形态/范围），核心歧义已收敛
