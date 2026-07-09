# 补全 doubao(+byteplus) model_list+models 全部官方信息

## Goal

字节火山引擎方舟。doubao 国内版 + byteplus 国际版两块。

- **doubao（国内）**：现 11 模型，research 全谱 18 模型（含 Seed 2.1 旗舰 + evolving 周更 + 2.0 全档 + 1.x legacy + 方舟聚合 7 个第三方）。补全 7 个新模型 + 修正 pricing source_url。
- **byteplus（国际）**：现 4 模型且 ID 格式错误（用国内 `doubao-seed-*` 前缀），research 确认国际版标准 ID 为 `seed-*` 前缀，全谱 9 模型（6 自营 + 3 聚合）。修正 ID 格式 + 补全 5 个新模型 + 修正 source_urls（当前误用国内 URL）+ 修正 models.default。

两块同 task 覆盖，各自独立改动。

## Research References

- [`research/doubao-models.md`](research/doubao-models.md) — research 全文（国内 model_list line 35 + 国际 line 166 + endpoints + ID 格式差异 caveat 7 + 第三方聚合范围差异）

## Requirements

### doubao（国内版）

#### 1. endpoints（3 端点，保留不动）

套餐端点（Agent/Coding Plan），与 research line 116-124 一致：

```json
"default": [
  {"protocol": "anthropic", "base_url": "https://ark.cn-beijing.volces.com/api/plan", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://ark.cn-beijing.volces.com/api/plan/v3", "client_type": "codex_tui"},
  {"protocol": "openai_responses", "base_url": "https://ark.cn-beijing.volces.com/api/plan/v3", "client_type": "codex_tui"}
]
```

#### 2. model_list.default（18 模型，`doubao-seed-*` + 第三方聚合 ID 格式）

按 research line 331-355 推荐清单。新增 7 项标 **(新)**：

```json
"default": [
  "doubao-seed-evolving",
  "doubao-seed-2-1-pro-260628",
  "doubao-seed-2-1-turbo-260628",
  "doubao-seed-2-0-code",
  "doubao-seed-2-0-pro",
  "doubao-seed-2-0-lite",
  "doubao-seed-2-0-mini",
  "doubao-seed-code",
  "doubao-seed-character",
  "doubao-seed-1.8",
  "doubao-seed-1.6",
  "minimax-m2.7",
  "minimax-m3",
  "glm-5.2",
  "deepseek-v4-flash",
  "deepseek-v4-pro",
  "kimi-k2.6",
  "kimi-k2.7-code"
]
```

新增项：`doubao-seed-evolving` / `doubao-seed-2-1-pro-260628` / `doubao-seed-2-1-turbo-260628` / `doubao-seed-2-0-mini` / `doubao-seed-character` / `doubao-seed-1.8` / `doubao-seed-1.6`。
保留项：现有 11 个全保留。删除项：无（Deprecated 模型如 `doubao-seed-1-8-251228` 本就未入 preset）。

#### 3. models.default（档位名 key → model id string）

```json
"models": {
  "default": {
    "default": "doubao-seed-2-0-code",
    "fast": "doubao-seed-2-0-mini",
    "thinking": "doubao-seed-evolving"
  }
}
```

- `default`：保持 `doubao-seed-2-0-code`（research line 113 首选建议，性价比 + Coding 优化 + 256k 上下文）
- `fast`：`doubao-seed-2-0-mini`（Mini 极速模型 → fast 档）
- `thinking`：`doubao-seed-evolving`（周级迭代最新模型，Coding+Agent 持续进化）

对齐 `Partial<Record<ModelSlot,string>>`。

#### 4. desc（保留 8 语言不动）

#### 5. source_urls（修正 pricing）

```json
"source_urls": {
  "docs": "https://www.volcengine.com/docs/82379",
  "pricing": "https://www.volcengine.com/docs/82379/1544106"
}
```

pricing 从 `https://www.volcengine.com/docs/6879`（旧通用产品页）→ `https://www.volcengine.com/docs/82379/1544106`（research line 15 模型价格页）。

---

### byteplus（国际版）

#### 1. endpoints（3 端点，保留不动）

research line 405 确认「当前配置已正确」：

```json
"default": [
  {"protocol": "anthropic", "base_url": "https://ark.ap-southeast.bytepluses.com/api/coding", "client_type": "claude_code"},
  {"protocol": "openai", "base_url": "https://ark.ap-southeast.bytepluses.com/api/plan/v3", "client_type": "codex_tui"},
  {"protocol": "openai_responses", "base_url": "https://ark.ap-southeast.bytepluses.com/api/plan/v3", "client_type": "codex_tui"}
]
```

#### 2. model_list.default（9 模型，`seed-*` 国际版 ID 格式）

按 research line 370-386 推荐清单。**ID 前缀从 `doubao-seed-*` 修正为 `seed-*`**（research caveat 1/7 line 162-164, 312）：

```json
"default": [
  "seed-2-0-pro",
  "seed-2-0-code-preview",
  "seed-2-0-lite",
  "seed-2-0-mini",
  "seed-1-8",
  "seed-1-6",
  "glm-5-2",
  "deepseek-v4-pro",
  "deepseek-v4-flash"
]
```

变更项：
- ID 格式修正（4 项）：`doubao-seed-2-0-pro` → `seed-2-0-pro`、`doubao-seed-2-0-code-preview` → `seed-2-0-code-preview`、`doubao-seed-2-0-lite` → `seed-2-0-lite`、`doubao-seed-2-0-mini` → `seed-2-0-mini`
- 新增（5 项）：`seed-1-8` / `seed-1-6` / `glm-5-2` / `deepseek-v4-pro` / `deepseek-v4-flash`

第三方聚合 ID 格式注意：国际版 GLM 为 `glm-5-2`（连字符分隔），国内版为 `glm-5.2`（点分隔），两者指向同一模型（research line 275）。

#### 3. models.default（修正 ID 格式）

```json
"models": {
  "default": {
    "default": "seed-2-0-pro",
    "fast": "seed-2-0-mini"
  }
}
```

- `default`：从 `doubao-seed-2-0-pro` → `seed-2-0-pro`（research line 210-211 首选建议，国际版标准 ID）
- `fast`：`seed-2-0-mini`（Mini 极速 → fast 档）

#### 4. desc（保留 8 语言不动）

#### 5. source_urls（修正：当前误用国内 URL）

```json
"source_urls": {
  "docs": "https://docs.byteplus.com/en/docs/ModelArk",
  "pricing": "https://docs.byteplus.com/en/docs/ModelArk/1544106"
}
```

当前误用 `https://www.volcengine.com/docs/82379`（国内 URL）+ `https://www.volcengine.com/docs/6879`（国内 URL）。修正为 BytePlus 国际版文档（research line 26-31）。

## Acceptance Criteria

- [ ] doubao model_list.default 含 18 模型（JSON 合法 + 无重复 + 排序按 research）
- [ ] byteplus model_list.default 含 9 模型 + 全部使用 `seed-*` 前缀（无 `doubao-seed-*` 残留）
- [ ] doubao models.default 含 default/fast/thinking 三档
- [ ] byteplus models.default default=`seed-2-0-pro`（非 `doubao-seed-2-0-pro`）
- [ ] doubao source_urls.pricing 修正为 82379/1544106
- [ ] byteplus source_urls 修正为 docs.byteplus.com 国际版
- [ ] endpoints 两块保留不动
- [ ] desc 两块保留不动
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] `yarn build` clean（前端无改动可跳）
- [ ] 不动 name/homepage/logo_url/client_type

## Out of Scope

- 上下文窗口数值字段补全（独立 task）
- STATIC_MODEL_IDS（独立 task）
- peak_hours / coding_plan 子分支（per CLAUDE.md 2026-07-08 决策，preset JSON 默认不带）
- 其他协议块（minimax/kimi/glm/deepseek 等独立协议）
- Deprecated 模型（research 标注的 `*-251228` / `*-250828` 等日期后缀版本，本就未入 preset）
- pricing 字段（cost 估算，独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json` 的 `protocols.doubao` + `protocols.byteplus`
- 数据来源：research 基于火山引擎官方文档 `volcengine.com/docs/82379`（国内）+ `docs.byteplus.com/en/docs/ModelArk`（国际）+ `llms-full.txt` 抓取
- doubao ID 格式：`doubao-seed-*`（国内前缀）
- byteplus ID 格式：`seed-*`（国际前缀，无 `doubao-` 前缀）
- 第三方聚合 ID 格式：国内 `glm-5.2`（点）vs 国际 `glm-5-2`（连字符），同模型不同命名
- research caveat：byteplus preset 当前用国内 ID 格式（`doubao-seed-2-0-pro`）可能国际端点不兼容（research line 312, 388）
