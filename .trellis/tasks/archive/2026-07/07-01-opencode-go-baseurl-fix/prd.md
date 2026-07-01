# PRD — opencode 协议预设 base_url 缺 /v1 致 fetch-models + 推理 404

> request_id=a88d75c541cc4ce6979070359978ff97 复现。fetch-models 探测 opencode 协议 → /zen/go/models 404。

## 根因 (调查定位)
- `src/pages/Platforms.tsx:397` `opencode` 协议预设 endpoint `base_url = "https://opencode.ai/zen/go"` **缺 `/v1` 后缀**
- `build_models_url` 默认拼 `{base}/models` → `/zen/go/models` 404
- 推理拼 `/chat/completions` → `/zen/go/chat/completions` **同样 404** (推理也坏, 非仅列模型)
- 实测正确端点: `https://opencode.ai/zen/go/v1/models` 200 ✅; `https://opencode.ai/zen/go/v1/chat/completions` 401 (路由存在, 鉴权另算)
- 对照 `opencode_zen` 协议预设 (Platforms.tsx:403) `https://opencode.ai/zen/v1` ✅ 正确 — 两协议端点结构不同 (/zen/v1 vs /zen/go/v1), 但都需 /v1 前缀

## 目标
`Platforms.tsx:397` base_url `https://opencode.ai/zen/go` → `https://opencode.ai/zen/go/v1`

## scope
- 单文件单行: `src/pages/Platforms.tsx:397`
- `opencode` 协议预设 endpoint base_url 加 `/v1` 后缀
- 注释补: 说明 /zen/go/v1 为 codex_tui OpenAI 兼容路由根 (区别 opencode_zen 的 /zen/v1)

## 非目标
- 不动 `opencode_zen` 协议预设 (已正确)
- 不改 build_models_url (默认拼 /models 正确, 问题在 base_url)
- 不处理 /zen/go 鉴权 (401 是 public key 不被 /zen/go 接受, 用户填真 key 即可)

## 验收
1. opencode 协议添加平台 → fetch-models 拉 `/zen/go/v1/models` 200
2. 推理请求走 `/zen/go/v1/chat/completions` (路由存在)
3. `yarn build` 0 err (纯前端单行改)
4. `check-i18n.mjs` 零缺失 (无新 key)

## 风险
- /zen/go 端点未来可能变 (opencode 迭代) — 本次按当前实测修
- codex_tui client_type 的 path 格式假设 /v1 前缀 — 需确认 codex_tui 不另拼 /v1 (否则双重 /v1)

## 调度
- 单行修复, 轻量模式 (单 subagent)
