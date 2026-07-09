# 百度千帆 Coding Plan Lite 调研

- **Query**: 百度千帆 Coding Plan Lite 真实 API 信息（base_url / provider_api_path / cp 独占模型 / client_type / 鉴权 / 与普通版区别）
- **Scope**: external（直接 probe qianfan.baidubce.com 真实 API 网关）+ internal（现有 preset 对照）
- **Date**: 2026-07-10
- **方法**: 直接对 `qianfan.baidubce.com` 网关发起 POST 探测，依据响应体（Anthropic FastAPI 校验 / BCE `invalid_iam_token` / `ResourceNotFound`）反推路由是否存在；外部公开文档页面（`cloud.baidu.com/doc/qianfan-api/*`）均为 JS 渲染 + 反爬，curl 不可读，未取得官方说明文本。

## 关键结论（TL;DR）

千帆 **存在独立的 coding plan 路由段 `/coding/`**，与普通 API 共用 `qianfan.baidubce.com` 主机但路径不同：

| 协议 | 普通版 base_url | Coding Plan base_url |
|---|---|---|
| anthropic | `https://qianfan.baidubce.com/anthropic` | `https://qianfan.baidubce.com/anthropic/coding` |
| openai | `https://qianfan.baidubce.com/v2` | `https://qianfan.baidubce.com/v2/coding` |

模式与 `glm_coding`（普通 `/api/paas/v4` → cp `/api/coding/paas/v4`）一致：在版本段前插入 `/coding/` 段。

## base_url

- **cp anthropic 兼容**：`https://qianfan.baidubce.com/anthropic/coding`
  - 探测证据：POST `/anthropic/coding/v1/messages` 返回 `{"detail":[{"type":"missing","loc":["body","model"],...}]}`（Anthropic Messages API FastAPI 校验），证明路由存在并解析到 Anthropic 协议处理器。
  - 对照：POST `/anthropic/v1/messages` 同样有效；POST `/anthropic` / `/anthropic/coding`（裸路径）返 `ResourceNotFound` —— 即 base_url 必须不带尾部 `/v1/messages`，由 aidog 的 `provider_api_path=/v1/messages` 拼接。
- **cp openai 兼容**：`https://qianfan.baidubce.com/v2/coding`
  - 探测证据：POST `/v2/coding/chat/completions` 返 `{"error":{"code":"invalid_iam_token",...}}`（BCE IAM 校验错误），证明路由存在；对照 `/v2/anything-else/chat/completions` 返 `ResourceNotFound`，排除通配匹配。
  - 普通版 openai base：`https://qianfan.baidubce.com/v2`（POST `/v2/chat/completions` 同样返 `invalid_iam_token`）。

## provider_api_path

- anthropic 协议：`/v1/messages`（Anthropic Messages API，与所有 anthropic 协议平台一致）。
- openai 协议：`/chat/completions`（千帆 v2 为 OpenAI Chat Completions 兼容）。

## cp 独占模型

- **未取得官方独占模型清单**（千帆官方文档 JS 渲染不可读，curl 搜索引擎被百度反爬拦截）。
- 推测（基于现有 preset `qianfan.model_list`）：cp lite 套餐主要面向 ERNIE 系列编程场景，可用模型与普通版重合度高，主力 `ernie-4.5-turbo`、`ernie-5.1`、`ernie-x1-turbo` 等。
- 现有 preset `qianfan.model_list.default`：`["ernie-5.1", "ernie-5.0", "ernie-4.5-turbo-vl", "ernie-4.5-turbo", "ernie-x1-turbo", "ernie-x1.1-preview"]`，`models.default.default = "ernie-4.5-turbo"`。
- `需要: 千帆官方 Coding Plan Lite 是否有独占 ERNIE 编程模型（如 ernie-x1-coder 之类），需查 console.bce.baidu.com 控制台套餐详情或付费文档。`

## client_type

- **anthropic 端点**：`claude_code`（与 Anthropic Messages API 配套，Claude Code CLI 直接消费）。
- **openai 端点**：`codex_tui`（与 OpenAI Chat Completions 配套，Codex TUI 消费）。
- 依据：与 `glm_coding` / `bailian_coding` / `kimi` 等同模式（双 endpoint：openai→codex_tui，anthropic→claude_code）。

## 鉴权

- **BCE IAM token**（`Authorization: Bearer <iam_token>` 或 BCE 签名）。
  - 探测证据：所有 openai 路由错误码均为 `invalid_iam_token`（BCE 标准 IAM 错误），非 OpenAI 风格 `invalid_api_key`。
  - anthropic 路由透传上游 `Error code: 401 - {'error': {'code': 'invalid_iam_token', ...}}`，同样 IAM 校验。
- **不是单一 API Key**：千帆沿用 BCE 体系，凭证签发走 AK/SK → access token（OAuth 模式）或控制台应用密钥；与 OpenAI 直填 API Key 模式不同。aidog `qianfan` preset 当前未设特殊鉴权字段，按平台通用 Bearer 注入即可。
- `需要: aidog 现有 qianfan adapter 是否已处理 BCE token 刷新（未在 src-tauri/src/gateway/adapter/ 见 qianfan.rs，说明走通用 anthropic/openai 转换路径，鉴权靠用户在 platform.extra 填完整 Bearer）。`

## 与普通版区别

| 维度 | 普通版 qianfan | Coding Plan Lite |
|---|---|---|
| base_url (anthropic) | `/anthropic` | `/anthropic/coding` |
| base_url (openai) | `/v2` | `/v2/coding` |
| 主机 | `qianfan.baidubce.com`（同） | 同 |
| 鉴权 | BCE IAM token（同） | 同 |
| 计费 | 按 token 量计 | 套餐订阅制（Coding Plan Lite 包月/包量，峰值可能倍率 —— 与 glm_coding peak_hours 类似） |
| client_type | 与 cp 一致 | 与普通一致（claude_code / codex_tui） |

唯一硬区别是 **URL 路径段 `/coding/`**。cp lite 是千帆套餐层概念（订阅 + 可能的高峰倍率），而非独立协议或独立鉴权。

## 来源（URL）

直接 probe（本调研一手证据，2026-07-10 取自网关响应）：
- `POST https://qianfan.baidubce.com/anthropic/coding/v1/messages` → Anthropic FastAPI 校验错误（路由存在）
- `POST https://qianfan.baidubce.com/anthropic/v1/messages` → 同上（普通版对照）
- `POST https://qianfan.baidubce.com/v2/coding/chat/completions` → `invalid_iam_token`（路由存在）
- `POST https://qianfan.baidubce.com/v2/chat/completions` → `invalid_iam_token`（普通版对照）
- `POST https://qianfan.baidubce.com/v2/anything-else/chat/completions` → `ResourceNotFound`（反证非通配）

官方文档入口（JS 渲染，curl 不可读，记录待人工/浏览器核对）：
- https://cloud.baidu.com/doc/qianfan-api/ — 千帆 API 文档总入口
- https://cloud.baidu.com/product/wenxinworkshop/ — 文心千帆工作台（含套餐）
- `需要: 浏览器打开 cloud.baidu.com/doc/qianfan-api/ 找「编程套餐 / Coding Plan Lite / Claude Code 接入」章节核对独占模型与计费倍率。`

内部参照（aidog 仓库）：
- `src-tauri/defaults/platform-presets.json:331-347`（现有 qianfan preset，anthropic/coding 已用）
- `src-tauri/defaults/platform-presets.json:91-130`（glm_coding 模板）
- `src-tauri/defaults/platform-presets.json:221-238`（bailian_coding 模板）
- `src-tauri/src/gateway/models/protocol.rs:57-58`（`Protocol::QianFan` serde `qianfan`）
- `src/domains/platforms/constants.ts:30`（`百度千帆 Coding Plan Lite` codingPlan label，待拆独立 key `qianfan_coding`）
- `CLAUDE.md`（项目方向：cp 拆独立协议 key，serde `qianfan_coding`）

## 结论（推荐字段值）

新增 `qianfan_coding` 独立协议，JSON 字段建议（与 glm_coding 同结构）：

```json
"qianfan_coding": {
  "is_coding_plan": true,
  "client_type": "claude_code",
  "endpoints": {
    "default": [
      {"protocol": "openai", "base_url": "https://qianfan.baidubce.com/v2/coding", "client_type": "codex_tui", "coding_plan": true},
      {"protocol": "anthropic", "base_url": "https://qianfan.baidubce.com/anthropic/coding", "client_type": "claude_code", "coding_plan": true}
    ]
  },
  "models": {
    "default": {"default": "ernie-4.5-turbo", "sonnet": "ernie-4.5-turbo", "opus": "ernie-5.1", "haiku": "ernie-x1-turbo"}
  },
  "model_list": {"default": ["ernie-5.1", "ernie-5.0", "ernie-4.5-turbo-vl", "ernie-4.5-turbo", "ernie-x1-turbo", "ernie-x1.1-preview"]},
  "name": {"en-US": "Baidu Qianfan Coding Plan Lite", "zh-Hans": "百度千帆 Coding Plan Lite", ...8 locale},
  "desc": {"en-US": "Baidu Qianfan Coding Plan Lite subscription endpoint (Anthropic & OpenAI compatible, ERNIE series)", "zh-Hans": "百度千帆编程套餐 Lite 端点（Anthropic / OpenAI 双协议兼容，ERNIE 系列）", ...8 locale},
  "source_urls": {
    "docs": "https://cloud.baidu.com/doc/qianfan-api/",
    "pricing": "https://cloud.baidu.com/product/wenxinworkshop/"
  },
  "homepage": "https://cloud.baidu.com",
  "logo_url": "baidu"
}
```

要点：
1. **双 endpoint**（openai + anthropic）—— 与 glm_coding 同；当前 `qianfan` preset 仅配了 anthropic 单条，cp 拆出后建议补 openai `/v2/coding` 双协议。
2. **base_url 区别仅在 `/coding/` 路径段**（已 probe 证实双侧路由）。
3. **client_type** 双值：openai→codex_tui，anthropic→claude_code；顶层 `client_type: "claude_code"`（与 bailian_coding 顶层 default 不同，bailian_coding 因仅单 anthropic 端点）—— 因 cp 双协议，顶层取主用（claude_code）。
4. **is_coding_plan: true** + 端点级 `coding_plan: true` flag。
5. **peak_hours 未填**：千帆官方是否对 cp lite 设高峰倍率未取得一手资料，`需要: 浏览器查控制台/套餐页确认`。若无需可 absent（=1.0）。
6. **模型列表** 暂复用普通版（同 ERNIE 系列），待官方资料核对是否有 cp 独占编程模型。
7. **Rust Protocol 枚举** 需加 `QianFanCoding` serde `qianfan_coding`（与 `GlmCoding` serde `glm_coding` 同模式）；前端 `PROTOCOLS` 加 `{ value: "qianfan_coding", label: "百度千帆 Coding Plan Lite", ... }`。

## Caveats / Not Found

- **千帆官方文档 JS 渲染 + 反爬**：`cloud.baidu.com/doc/qianfan-api/*` 与百度搜索均无法 curl 抓取正文，独占模型清单 / 套餐计费倍率 / peak_hours 配置未取得一手官方文本。需 main 协调用户用浏览器查 console.bce.baidu.com/qianfan 套餐详情页确认：
  - `需要: cp lite 是否有独占编程模型（ernie-*-coder 之类）？`
  - `需要: cp lite 是否有高峰倍率（glm_coding 模式的 peak_hours）？`
  - `需要: cp lite 订阅含哪些 ERNIE 模型、是否限制并发或日 quota？`
- **OpenAI 兼容 cp 端点 `/v2/coding`** 为本次 probe 一手发现，未在现有 preset 出现（当前 qianfan preset 仅 anthropic 单条）；逻辑上应存在（与 anthropic/coding 对称），但无官方文档佐证。建议接入前用真实凭证试调一次。
- 本调研仅证明路由存在（非 404），未验证 cp 套餐鉴权后的实际模型推理是否成功 —— 需真实 BCE IAM token 才能闭环。
```
