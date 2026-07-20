# CPA 解析器漏认 OAuth 凭据格式 — PRD

## 目标
拖入含 CLIProxyAPI OAuth 凭据文件(`type`+`access_token`+`model_aliases` 结构)的 zip/文件夹/单文件作 source 时, 解析器识别为 OAuth provider 不再全跳过; 多个 OAuth 凭据各成独立平台不被合并。

## 用户价值
CLIProxyAPI 多账号负载均衡场景: 用户导出含 N 个 xai/codex/claude OAuth 凭据的 zip, 一次性导入为 N 个独立平台(各 email/token)。当前全跳过或(修 #1 后)被合成 1 个, 无法多账号。

## 边界
**范围内**:
- `parse_single_file` 加 OAuth 单文件识别分支(JSON 含 `type` 可 parse 为 CpaOAuthType + 非空 `access_token` → OAuth provider)
- 抽 `parse_oauth_json(content) -> Option<Vec<CpaProvider>>` 复用(scan_auth_dir + parse_single_file 同逻辑, DRY)
- `deduplicate_providers` OAuth 段 dedup key 改 `(oauth_type, name/email)`, 避免多凭据按空 base_url 合并
- 补单测: OAuth 单文件解析 + OAuth 多凭据 dedup 不合并

**范围外**:
- 不改 mapper.rs(OAuth provider 映射逻辑正确)
- 不改 cpa_import.rs 命令签名
- 不改前端(ParseResult 结构不变)
- 不改 scan_auth_dir 公开行为(仅内部复用抽函数)

## 验收标准
- [ ] 拖入仅含 `{"type":"xai","email":"a@b","access_token":"tok","model_aliases":[{"name":"grok-1","alias":"g1"}]}` 的 JSON → 解析为 1 个 OAuth provider(xai, name=a@b), 不跳过
- [ ] 拖入含 10 个不同 email 的 xai OAuth JSON 的 zip → 10 个独立 provider(不合并为 1)
- [ ] 拖入仅含 `{"type":"xai"}` 无 access_token 的 JSON → 跳过(原因明确, 不崩)
- [ ] 拖入含 CPA config 段(`gemini-api-key` 等)的 yaml/json → 原逻辑不变(回归)
- [ ] scan_auth_dir(auth_dir 路径)行为不变(回归, 抽函数复用不破现有)
- [ ] cargo test 全绿(含新测试)
- [ ] cargo clippy 零 warning
- [ ] mapper.rs 零改动(确认 diff 不沾)

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/调度: task.json (`skein.py subtask list cpa-parse-no-provider`)
