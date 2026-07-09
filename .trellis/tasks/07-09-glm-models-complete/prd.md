# 补全 glm(+glm_en) model_list+models 全部官方信息

## Goal

智谱 GLM。glm（国内 bigmodel.cn）+ glm_en（国际 z.ai）同源镜像，两协议 model_list 完全一致（仅域名异 `.cn` vs `.com`）。

- **补全**：现 8 模型，research 推荐补 2 个在售高价值项（`glm-4.7-flashx` 轻量高速版 + `glm-4.5-airx` Air 极速版），最终 10 模型
- **不动**：endpoints（普通分支，per CLAUDE.md 2026-07-08 coding_plan 去重决策）、models.default.default（`glm-5.2` 旗舰保持）
- **补 models.default 多档**：fast 档（`glm-4.7-flashx`）

两协议同 task 覆盖，model_list 完全一致。

## Research References

- [`research/glm-models-endpoints.md`](research/glm-models-endpoints.md) — research 全文（国内表 line 21-40 + 国际表 line 48-65 + 遗漏分析 line 112-123 + 推荐清单 line 155-181 + caveats line 245-252）

## Requirements

### 1. endpoints（2 块各 2 端点，保留不动）

research line 182-193 确认「维持现状（普通分支），不引入 coding_plan 子分支（遵循 2026-07-08 去重决策）」。

**glm（国内）**：
```json
"default": [
  {"protocol": "openai", "base_url": "https://open.bigmodel.cn/api/paas/v4", "client_type": "codex_tui", "coding_plan": false},
  {"protocol": "anthropic", "base_url": "https://open.bigmodel.cn/api/anthropic", "client_type": "claude_code", "coding_plan": false}
]
```

**glm_en（国际）**：
```json
"default": [
  {"protocol": "openai", "base_url": "https://api.z.ai/api/paas/v4", "client_type": "codex_tui"},
  {"protocol": "anthropic", "base_url": "https://api.z.ai/api/anthropic", "client_type": "claude_code"}
]
```

### 2. model_list.default（10 模型，glm + glm_en 完全一致）

按 research line 161-174 推荐清单（保守，仅加 2 个高价值项）。新增 2 项标 **(新)**：

```json
"default": [
  "glm-5.2",
  "glm-5.1",
  "glm-5",
  "glm-5-turbo",
  "glm-4.7",
  "glm-4.7-flashx",
  "glm-4.7-flash",
  "glm-4.6",
  "glm-4.5-air",
  "glm-4.5-airx"
]
```

- **新增**：`glm-4.7-flashx`（4.7 轻量高速版，research line 115「同基座轻量高速版，与 glm-4.7-flash 免费版区分明显」）、`glm-4.5-airx`（4.5 Air 极速版，research line 116「与已有 glm-4.5-air 配对，符合轻量+高速双档惯例」）
- **保留**：现有 8 模型全保留。`glm-5.1`/`glm-5` 为历史 id（调用自动迁 5.2，research line 130-131），保留无害（向后兼容用户配置）
- **不加**（research line 117-122）：`glm-4.5`/`glm-4.5-x`（即将下线）、`glm-4.5-flash`（免费+即将下线）、`glm-4-long`（输出仅 4K）、`glm-4-*-250414`（GLM-4 老 250414 系列能力天花板低）、`glm-4-plus` 等 legacy（max_tokens 仅 4095）、`glm-4-32b-0414-128k`（z.ai 独有，国内端点不可用，两协议不一致故不加）

### 3. models.default（档位名 key → model id string，glm + glm_en 一致）

```json
"models": {
  "default": {
    "default": "glm-5.2",
    "fast": "glm-4.7-flashx"
  }
}
```

- `default`：保持 `glm-5.2`（research line 197 维持，在售旗舰，1M 上下文 + Coding SOTA）
- `fast`：`glm-4.7-flashx`（轻量高速版 → fast 档，research line 28「同 4.7 基座，对标国产前端/写作，低延迟」）

对齐 `Partial<Record<ModelSlot,string>>`。注意选 `flashx`（付费高速版）而非 `flash`（免费普惠版），因 fast 档定位高速。

### 4. desc（保留 8 语言不动，两块均不动）

### 5. source_urls（保留）

- glm：`docs: https://docs.bigmodel.cn/cn/guide/start/quick-start` + `pricing: https://open.bigmodel.cn/pricing` — 保留
- glm_en：`docs: https://docs.z.ai/guide/start/quick-start` + `pricing: https://z.ai/pricing` — 保留

## Acceptance Criteria

- [ ] glm model_list.default 含 10 模型（含新增 glm-4.7-flashx + glm-4.5-airx）
- [ ] glm_en model_list.default 与 glm 完全一致（10 模型）
- [ ] glm + glm_en models.default 含 default/fast 两档
- [ ] 不含即将下线模型（glm-4.5 / glm-4.5-x / glm-4.5-flash）
- [ ] endpoints 两块保留不动
- [ ] desc/source_urls/name 两块保留不动
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] `yarn build` clean（前端无改动可跳）

## Out of Scope

- GLM-5.2 1M 上下文（`[1m]` 后缀启用，非独立 model id，research line 248）
- 上下文窗口数值字段
- STATIC_MODEL_IDS（passthrough.rs）
- peak_hours / coding_plan 子分支（per CLAUDE.md 2026-07-08 决策，preset JSON 默认不带；用户级 `platform.extra` 可手工启用 cp 端点 `https://open.bigmodel.cn/api/coding/paas/v4`）
- `glm-4-32b-0414-128k`（z.ai 独有，两协议不一致故不加，research line 123/251）
- pricing 字段（cost 估算，独立 task）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json` 的 `protocols.glm` + `protocols.glm_en`
- 数据来源：research 基于 `https://docs.bigmodel.cn/llms-full.txt`（39578 行快照）+ `https://docs.z.ai/llms-full.txt`（18421 行快照），均 2026-07-09 抓取
- ID 格式：`glm-<ver>-<variant>`（如 `glm-5.2`、`glm-4.7-flashx`），无日期后缀
- 两协议同源：model_list 完全一致（research line 176-180），仅域名/base_url 不同
- `glm-4.7-flash`（免费普惠版）vs `glm-4.7-flashx`（轻量高速版付费）：两者能力档不同（research line 134/249），preset 应同时提供
- `glm-5.1`/`glm-5`：历史 id，调用自动迁 `glm-5.2`（research line 130-131），保留为用户既有配置兼容
- Coding Plan 端点（research line 80-93）：仅 OpenAI 协议有 `/coding/` 路径段，Anthropic 协议同 URL 靠 API Key 鉴权识别套餐；按 CLAUDE.md 决策 preset 默认不带 cp 子分支
