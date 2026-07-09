# JSON schema 扩展 + 3 独立 cp key（research 后修订：5→3）

## Goal
`src-tauri/defaults/platform-presets.json` 扩展：① 现有 61 协议顶层各加 `keywords` / `color` / `codingKeyPrefixes`（从 TS 常量 PROTOCOLS/PROTOCOL_COLORS 迁）；② 加 **3** 独立 coding plan 协议 key（`kimi_coding` / `qianfan_coding` / `xiaomi_mimo_coding`），与 glm_coding 同模式，含真实 cp endpoints/models。

**5→3 修订依据**：5 平台 cp 调研全回（2026-07-10），minimax（国内）+ minimax_en（国际）research 证伪独立协议（Token Plan 共用 endpoint，无 `/coding/` 路径，纯计费层）。仅 kimi/qianfan/xiaomi_mimo 3 平台 5 维度全异于普通版，值得独立协议。

## research 汇总（5 平台）

| 协议 | 结论 | 独立协议 | 依据 |
|---|---|---|---|
| kimi | 独立 `api.kimi.com/coding/` + 独占 `kimi-for-coding`/`-highspeed` + Subscription key（与 API key 互斥） | ✅ YES | research/kimi-coding.md |
| qianfan | `/coding/` 路径段（anthropic `/anthropic/coding` + openai `/v2/coding`，probe 实证路由存在）+ BCE IAM 鉴权 | ✅ YES | research/qianfan-coding.md |
| xiaomi_mimo | 独立 host `token-plan-cn.xiaomimimo.com`（cn/sgp/ams 三集群）+ tp- 前缀 key + `api-key:` header（非 x-api-key 非 Bearer） | ✅ YES | research/xiaomi-mimo-coding.md |
| minimax（国内） | Token Plan 共用 endpoint，无 `/coding/`，纯计费层 | ❌ NO | research/minimax-coding.md |
| minimax_en（国际） | endpoint probe `api.minimax.io/v1/coding/` → 404，无独立 cp 路径 | ❌ NO | research/minimax-en-coding.md |

## Requirements

### R1 现协议 +3 字段
- R1.1 每协议顶层加 `keywords: [string]`（搜索词，从 constants.ts PROTOCOLS 逐条迁，禁遗漏 61 条）。
- R1.2 每协议顶层加 `color: "#hex"`（从 PROTOCOL_COLORS 迁）。
- R1.3 `codingKeyPrefixes: [string]`（仅 xiaomi_mimo 系 `tp-`，余协议 absent = 默认 []）。
- R1.4 3 独立 cp key 也需补这 3 字段。
- R1.5 json.load 等价硬门禁（改前后 Python json.load 字节级等价，仅格式变）。

### R2 3 独立 cp key
- R2.1 加 `kimi_coding` / `qianfan_coding` / `xiaomi_mimo_coding`，各含：
  - `is_coding_plan: true`
  - `client_type`: 双档 `codex_tui` + `claude_code`（与 glm_coding 同）
  - `endpoints`: research 实证 base_url（见各 research/*.md 推荐值）
    - **kimi_coding**：OpenAI `https://api.kimi.com/coding/v1` + Anthropic `https://api.kimi.com/coding/`（末尾斜杠，Kimi 官方约定）
    - **qianfan_coding**：OpenAI `https://qianfan.baidubce.com/v2/coding` + Anthropic `https://qianfan.baidubce.com/anthropic/coding`
    - **xiaomi_mimo_coding**：host `token-plan-cn.xiaomimimo.com`（三集群 cn/sgp/ams，base_url 路径结构与普通版一致仅换 host）
  - `models` / `model_list`: research 实证独占模型
    - **kimi_coding**：`["kimi-for-coding", "kimi-for-coding-highspeed"]`（固定别名，禁抄 k2.7-code 明文版本号会 404）
    - **qianfan_coding**：复用普通版 ERNIE 列表（cp 独占模型未公开，`需要:` 用户核对套餐详情页）
    - **xiaomi_mimo_coding**：与普通版共享 `mimo-v2.5-pro`/`mimo-v2.5`（无独占，区别在计费 Credits 配额制 + 0.8x 低峰折扣）
  - `name`: 多 locale map（8 locale，与 glm_coding 同结构）
  - `desc` / `source_urls` / `homepage` / `logo_url` / `color` / `keywords`
  - `peak_hours`: 3 协议均 absent（=1.0）；xiaomi 有 0.8x 低峰折扣但非 peak_hours 模型
- R2.2 serde key = JSON key（snake_case，与 Rust Protocol 枚举 serde 对齐，见 child2 protocols-rust-enum）。
- R2.3 minimax / minimax_en **不加 cp key**（research 证伪）：普通版单条保持，endpoint 级 `coding_plan: false` flag 仍可手工标。
- R2.4 **鉴权特例 caveats（本 task 只写 JSON，鉴权重构属 child2 Rust 层，此处仅记录）**：
  - xiaomi_mimo_coding：`api-key: <key>` header，Rust headers.rs 对 Anthropic 端点需补特例（`需要:` 真实 tp- key 验证 x-api-key 兼容性）
  - qianfan_coding：BCE IAM token（AK/SK 体系），非 OpenAI api_key 风格
  - kimi_coding：Subscription key（与 API key 互斥，同一 Bearer header）
- R2.5 minimax_en research 副产物：公开 quota 端点 `GET /v1/token_plan/remains`（国内 `api.minimaxi.com` 同路径）。**本 task 不接 quota**（独立任务判断），仅 research 记录。

### R3 research（已完成）
- R3.1 ✅ 5 平台 cp 调研全回，产物在 `research/{kimi,qianfan,xiaomi-mimo,minimax,minimax-en}-coding.md`。
- R3.2 ✅ research 落 task research/ 目录，本 PRD 引用。

## `需要:` 遗留（main 转用户，不阻塞 implement）
1. qianfan cp 是否有独占编程模型（ernie-*-coder）？现复用普通版 ERNIE 列表。
2. qianfan cp 是否有 peak_hours（glm_coding 3x 模式）？现 absent。
3. qianfan 套餐订阅含哪些模型/并发/日 quota？建议用户浏览器开 `console.bce.baidu.com/qianfan` 核对。
4. xiaomi_mimo Anthropic 端点是否兼容 `x-api-key`？官方文档未明，建议真实 tp- key 验证后再定路由层特例。
5. qianfan OpenAI cp 端点 `/v2/coding` 为 probe 一手发现，无官方文档，接入前用真实凭证闭环测试。

## Acceptance
- [ ] JSON 64 协议（原 61 + 3 cp），各含 keywords/color（codingKeyPrefixes 按需）
- [ ] 3 cp key 真实 endpoints/models（research 支撑，非猜测）
- [ ] json.load 等价（改前后字节级）
- [ ] 主仓零改动（worktree 内）

## Dependencies
无前置（DAG 首节点）。下游：protocols-rust-enum（+3 变体，非 5）/ protocols-frontend-derive。
