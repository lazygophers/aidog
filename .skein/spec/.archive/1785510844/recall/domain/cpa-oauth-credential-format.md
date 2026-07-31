---
title: CPA OAuth 凭据格式（CLIProxyAPI）
layer: recall
category: domain
keywords: [cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入]
source: cpa-parse-no-provider
authored-by: skein-memory
---

# CPA OAuth 凭据格式（CLIProxyAPI）

何时被读: 改 CPA 导入解析器 / 加新 OAuth 类型 / 处理 CLIProxyAPI 多账号凭据时
谁读: 写 cpa_import 模块 / 处理 OAuth 凭据映射的开发者

## 格式结构

CLIProxyAPI OAuth 凭据 JSON(auth-dir 文件 / 导出 zip 内):
```json
{
  "type": "xai",
  "email": "a@b.com",
  "access_token": "...",
  "refresh_token": "...",
  "model_aliases": [
    {"name": "grok-1", "alias": "g1"}
  ]
}
```

不同于 CPA config stub(6 个顶层 key: gemini-api-key / interactions-api-key / codex-api-key / claude-api-key / openai-compatibility / vertex-api-key)。需单独识别。

## 识别逻辑

- `parse_oauth_json(content) -> Option<Vec<CpaProvider>>`(parser.rs):
  - 非 JSON / type 不可识别 → None
  - 缺 access_token → None
  - 命中 → 返 1 个 provider(`source_segment=OAuth`, `base_url=""`, `oauth_type=Some(...)`)
- `is_oauth_credential(content) -> bool` 探测(仅看 type, 不要求 token)→ 区分「非 OAuth」vs「OAuth 缺 token」给独立错误文案

## 多账号语义（CLIProxyAPI）

- 同一 OAuth 类型(如 xai)可有多个凭据(各 email 不同)→ **各自独立平台**(负载均衡)
- dedup key 用 `(source_segment, name/email)`, **禁用** base_url(全空会撞, 见 [[auto-fix-downgrade-35]])
- mapper.rs 用 `name`(=email)做平台名, `oauth_type` 路由协议(xai→CpaGrok / claude→Anthropic / codex→Codex / kimi→Kimi / vertex→CpaVertex / aistudio→CpaAistudio / antigravity→CpaAntigravity)

## OAuth 类型枚举（CpaOAuthType）

codex / claude / kimi / xai / vertex / aistudio / antigravity(parser.rs::CpaOAuthType::parse_oauth_type, 大小写不敏感)。加新类型需同步此枚举 + mapper.rs::resolve_protocol OAuth 分支。

## Cross-ref

- `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser.rs:496-522` OAuthCredential 结构
- `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser.rs:532-565` parse_oauth_json
- `src-tauri/crates/aidog_core/src/gateway/cpa_import/mapper.rs:86-95` OAuth 协议路由
- 关联 [[parser-multi-path-format-symmetry]](两路径识别对称)
- 关联 [[auto-fix-downgrade-35]](多凭据 dedup key)
