# MITM 链路全诊断: 白名单匹配 + log 完整 + 路由触发 + token 解析 + h2 cancel

## Goal

用户 request_id=b1920a5c03c644c3a70917311ec126d8 (open.bigmodel.cn:443) 日志记录过少 (url 仅 CONNECT target, body/header/response 全 0, 无 token 解析), 期望命中 `.cn` suffix 走 mitm 解密 + 完整记录 + token 用量。诊断已定位根因 + 接管原 mitm-h2-cancel (已归档) 的 h2 stream cancel 子问题。

## 根因 (已诊断, evidence-based)

**核心 bug**: 白名单 suffix 前导点 + format! 双点致永不命中。

- DB 现状: `mitm_whitelist` 含用户规则 `.cn` (rule_type=suffix, source=user) — 带前导点
- 匹配逻辑 `src-tauri/src/gateway/mitm/whitelist.rs:152`:
  ```rust
  "suffix" => host == rule_value || host.ends_with(&format!(".{rule_value}")),
  ```
  - rule_value=`.cn` → `format!(".{rule_value}")` = `..cn` (双点) → `host.ends_with("..cn")` 永远 false
  - `host == ".cn"` 也 false
- 后果: `.cn` 不命中 → `mitm_candidate=false` (`connect.rs:134`) → 走 `spawn_blind_relay` (盲转不解密) → CONNECT 元数据是唯一记录, body/header/response 全空, 无 token 解析
- evidence: `b1920a5c` 行 url=`open.bigmodel.cn:443`, request_body/headers/response_body 全 length=0, token=0/0

## Requirements

### #1 suffix 前导点双端修 (核心)

**后端** (`whitelist.rs::matches_rule` suffix 分支):
- 对 rule_value strip 前导点 (循环 strip 所有前导 `.` 防止 `..cn` 这类输入)
- 保留原 Clash DOMAIN-SUFFIX 语义: `host == rule_value || host.ends_with(format!(".{rule_value}"))`
- 即: 标准化 rule_value 后再做匹配, 用户写 `.cn` / `cn` / `..cn` 都等价 `cn` 命中

**前端** (`MitmConfig.tsx::handleAdd`):
- 用户输入 newPattern 时 strip 前导点 (防脏数据入 db)
- 与后端容错双保险 (API 直接写库仍能匹配)

**测试** (`whitelist.rs` test 模块):
- 新增: `suffix_leading_dot_matches` — `.cn` rule 命中 `open.bigmodel.cn`
- 新增: `suffix_multi_leading_dot_normalized` — `..cn` 也归一化命中
- 保留现有 suffix 测试不回归

### #2 修后验证 open.bigmodel.cn 走 mitm 完整流程

修 #1 后 (用户 dev 实测):
- `.cn` 命中 → `mitm_candidate=true` → spawn mitm 路径 → 解密 TLS → 解析请求/响应
- 期望 proxy_log 字段填充: `request_url` (完整含 path/query), `request_body`/`request_headers`, `response_body`, `input_tokens`/`output_tokens`
- token 解析 + 用量通知 (notification) 生效

**main 验证手段** (用户 dev 跑后):
```sql
SELECT substr(id,1,12), status_code, substr(request_url,1,80), length(request_body), length(response_body), input_tokens, output_tokens
FROM proxy_log WHERE request_url LIKE '%bigmodel%' ORDER BY created_at DESC LIMIT 3;
```
应见完整 url + 非空 body + token > 0。

### #3 h2 stream CANCEL (条件性, 修 #2 后若撞)

#2 验证时若 open.bigmodel.cn (h2 上游) 撞 `HTTP/2 stream ... CANCEL (err 8)`:
- 接管原 `07-05-mitm-h2-cancel-real-rootcause` (已归档, 含 connect.rs/test_e2e_mitm.rs 探索性改动 commit f28c4503)
- 根因候选 (按概率):
  1. mitm 解密后 h2 直通 (connect.rs auto Builder) 流控/超时
  2. passthrough 上游响应流被中途切断 (stream.rs)
  3. MITM TLS ALPN 选 h2 后流处理
- #2 验证拿到 client error + proxy_log status + 上游响应部分返回情况后再定根因

## Acceptance Criteria

- [ ] `matches_rule` suffix 分支 strip 前导点, 单测覆盖 `.cn`/`..cn`/`cn` 三种输入均命中
- [ ] `MitmConfig.tsx handleAdd` strip 前导点
- [ ] `cargo test --lib whitelist` 全绿
- [ ] `yarn build` 通过
- [ ] 用户 dev 实测: open.bigmodel.cn 走 mitm, proxy_log 完整字段 (url/body/header/token) — **用户验收**
- [ ] (条件) 若撞 h2 cancel, 定位根因 + 修复 + 用户验收 CANCEL 消失
- [ ] spec 沉淀: suffix 匹配契约 (前导点容错语义)

## Definition of Done

- #1 改完 + 测试绿 + build 绿
- #2 用户 dev 实测确认 proxy_log 完整
- (#3 若触发) CANCEL 消失
- spec sediment: whitelist 匹配契约更新

## Out of Scope

- 已正确的 modal portal 化 (另一 task 已闭环)
- mitm 白名单其他 rule_type (domain/keyword/ipcidr) — 不动
- NO_PROXY=*.cn (reqwest 客户端层 env, 与代理服务端 mitm 白名单无关)
- 全局 css transform 根源 (modal task 范围)

## Technical Notes

- 核心文件: `src-tauri/src/gateway/mitm/whitelist.rs:141-170` (matches_rule) + `src-tauri/src/gateway/mitm/whitelist.rs:86-130` (matches_host)
- 决策点: `src-tauri/src/gateway/proxy/connect.rs:133-175` (mitm_candidate 分流)
- blind_relay: `spawn_blind_relay` (connect.rs 后续, 不解密, 字节透传)
- 前端: `src/components/settings/MitmConfig.tsx::handleAdd` (~行 128)
- db: `~/.aidog/aidog.db`, 表 `proxy_log` (列 request_url/request_body/request_headers/response_body/input_tokens/output_tokens 等) + `mitm_whitelist` (列 host_pattern/rule_type/enabled/source)
- 原 task 归档: `.trellis/tasks/archive/2026-07/07-05-mitm-h2-cancel-real-rootcause/` (含 connect.rs/test_e2e_mitm.rs 探索改动)
- 用户裁定: 全修 #1+#2+#3, suffix 双端修 (后端容错 + 前端规范化)
