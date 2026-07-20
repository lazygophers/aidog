# 调研收敛 — CPA 解析器漏认 OAuth 凭据格式

## 现象
用户拖 1.zip 作 source 拖入 CpaImportModal → 10 个 `xai-tmp*@bytehub.de5.net.json` 全跳过, 原因「不是 CPA 配置文件（无任何 CPA provider 段）」。

## 根因
`xai-tmp*.json` 是 CLIProxyAPI 的 **OAuth 凭据文件**(同 auth-dir JSON 格式):
- 顶层字段: `type`(如 `"xai"`) + `email` + `access_token` + `refresh_token` + `model_aliases[].name/alias`
- aidog 已有 `OAuthCredential` 结构(parser.rs:496-511)用于 auth-dir 扫描

但 `parse_single_file`(parser.rs:169) 走 `is_cpa_config()`(parser.rs:156) 只认 6 个 CPA config 段:
`gemini-api-key` / `interactions-api-key` / `codex-api-key` / `claude-api-key` / `openai-compatibility` / `vertex-api-key`

OAuth 凭据 JSON 无任一上述顶层 key → stub 判定 false → 报「无任何 CPA provider 段」。

OAuth 格式仅 `scan_auth_dir`(parser.rs:525, auth_dir 参数路径)识别。用户拖 zip 作 source(非 auth_dir) → 不触发 OAuth 解析。

## 次生 bug(修主 bug 后暴露)
`deduplicate_providers`(parser.rs:641) OAuth 段 dedup key = `(source_segment, base_url)`。OAuth provider `base_url` 全空(parser.rs:563 注释「OAuth 平台 base_url 由后续映射确定」)→ 10 个 xai 凭据 dedup key 全 `(OAuth, "")` 全撞 → 只留首个 + 后续 merge_models 丢 9 个 access_token。

用户期望: 每个 OAuth 凭据 = 独立平台(CLIProxyAPI 多账号负载均衡, 不同 email/token)。

## 下游影响面
- `mapper.rs::resolve_name`(mapper.rs:168) OAuth 段用 `name`(=email) 做平台名 — 确认 OAuth provider 各凭据 name 不同(各 email 不同)→ 修 dedup 后各平台独立可辨
- `mapper.rs::resolve_protocol`(mapper.rs:86) OAuth 段用 `oauth_type` 路由(xai→CpaGrok) — 不受影响
- `scan_auth_dir`(parser.rs:525) 现有 auth_dir 路径同结构 — 抽公共 `parse_oauth_json` 复用(DRY + 同 bug 一起修)

## 不改
- mapper.rs(OAuth provider 映射逻辑正确, 无需动)
- cpa_import.rs 命令入口(签名不变)
- 前端 CpaImportModal(解析返回结构 ParseResult 不变)

## 引用
- parser.rs:156-163 `is_cpa_config` 判定
- parser.rs:169-214 `parse_single_file` source 路径
- parser.rs:496-522 `OAuthCredential` / `OAuthModelAlias` 结构
- parser.rs:525-576 `scan_auth_dir` auth_dir 路径
- parser.rs:610-658 `deduplicate_providers`
- 上游 https://github.com/router-for-me/CLIProxyAPI (OAuth 凭据格式出处)
