# Research: Anthropic Claude 官方全模型清单 vs aidog preset

- **Query**: 查 Anthropic Claude 全部官方模型清单，判现 aidog preset 遗漏了哪些
- **Scope**: external（官方文档）+ internal（preset 比对）
- **Date**: 2026-07-08
- **Sources**:
  - https://docs.anthropic.com/en/docs/about-claude/models/overview （「Latest models comparison」表）
  - https://docs.anthropic.com/en/docs/about-claude/model-deprecations （「Model status」+ 「Deprecation history」表）

---

## 1. 官方全模型清单（按当前状态）

### 1.1 当前旗舰（General Available，最新一代，2026-07-08）

引自 overview 「Latest models comparison」表：

| 显示名 | Claude API ID | 别名 (alias) | 状态 | 上下文窗口 | Max output | 定价 (input/output MTok) |
|---|---|---|---|---|---|---|
| Claude Fable 5 | `claude-fable-5` | `claude-fable-5` | Active | 1M tokens | 128k | $10 / $50 |
| Claude Opus 4.8 | `claude-opus-4-8` | `claude-opus-4-8` | Active | 1M tokens | 128k | $5 / $25 |
| Claude Sonnet 5 | `claude-sonnet-5` | `claude-sonnet-5` | Active | 1M tokens | 128k | $3 / $15（intro $2 / $10 至 2026-08-31）|
| Claude Haiku 4.5 | `claude-haiku-4-5-20251001` | `claude-haiku-4-5` | Active | 200k tokens | 64k | $1 / $5 |

> 注：自 Claude 4.6 起的模型 ID 用「无日期格式」（dateless，pinned snapshot，非 evergreen pointer）。`claude-sonnet-5` / `claude-opus-4-8` / `claude-fable-5` 均为 dateless 完整 ID，**不是别名**。Haiku 4.5 是最后一代仍带日期后缀的 ID，`claude-haiku-4-5` 是其便利 alias，解析到 `claude-haiku-4-5-20251001`。

### 1.2 仍 Active 的前代模型（仍可调用，但非旗舰）

引自 deprecations 「Model status」表（overview 旗舰表未列）：

| API model name | 代次 | 状态 | 最早退役日 |
|---|---|---|---|
| `claude-opus-4-7` | Opus 4.7 | Active | ≥ 2027-04-16 |
| `claude-opus-4-6` | Opus 4.6 | Active | ≥ 2027-02-05 |
| `claude-opus-4-5-20251101` | Opus 4.5 | Active | ≥ 2026-11-24 |
| `claude-sonnet-4-6` | Sonnet 4.6 | Active | ≥ 2027-02-17 |
| `claude-sonnet-4-5-20250929` | Sonnet 4.5 | Active | ≥ 2026-09-29 |
| `claude-haiku-4-5-20251001` | Haiku 4.5 | Active | ≥ 2026-10-15 |

### 1.3 Deprecated / Retired（不可靠或不可调，禁列入默认）

| API model name | 状态 | 退役日 |
|---|---|---|
| `claude-opus-4-1-20250805` | Deprecated（2026-06-05）| 2026-08-05 |
| `claude-opus-4-20250514` | Retired | 2026-06-15 |
| `claude-sonnet-4-20250514` | Retired | 2026-06-15 |
| `claude-3-7-sonnet-20250219` | Retired | 2026-02-19 |
| `claude-3-5-sonnet-20240620` / `-20241022` | Retired | 2025-10-28 |
| `claude-3-opus-20240229` | Retired | 2026-01-05 |
| `claude-3-5-haiku-20241022` | Retired | 2026-02-19 |
| `claude-3-haiku-20240307` | Retired | 2026-04-20 |
| `claude-3-sonnet-20240229` / `claude-2.x` / `claude-1.x` / `claude-instant-*` | Retired | 2024-11 ~ 2025-07 |

### 1.4 Limited Availability / Research（非通用 API，禁列入默认）

| API model name | 状态 | 说明 |
|---|---|---|
| `claude-mythos-5` | Limited Availability | Project Glasswing，仅受邀客户，禁自服务注册；与 Fable 5 同规格同价位，defensive cybersecurity 用途 |
| `claude-mythos-preview` | Retiring 2026-06-30 | Project Glasswing 内部预览，仅受邀 |

> Mythos 系列不在标准 Claude API 通用目录里，普通 API key 调不通，**不应进 preset 默认清单**。

### 1.5 区域变体（EU 等）

官方文档无 `-eu` 或区域特供模型 ID。Bedrock / Google Cloud 仅有 endpoint 级别（global / regional / multi-region）的路由差异，模型 ID 本身统一。**Claude API 无 EU 变体需列**。

---

## 2. 现 preset vs 官方对比

### 现 preset（`src-tauri/defaults/platform-presets.json` `protocols.anthropic.model_list.default`）

```
claude-opus-4-8     ✅ 仍旗舰
claude-fable-5      ✅ 仍旗舰
claude-sonnet-4-6   ⚠️  仍 Active 但已被 Sonnet 5 取代为旗舰
claude-haiku-4-5    ✅ 仍旗舰（alias 形式，等价 claude-haiku-4-5-20251001）
```

### 遗漏项

| 遗漏模型 | 严重度 | 说明 |
|---|---|---|
| **`claude-sonnet-5`** | 🔴 高 | 当前 Sonnet 旗舰，1M 上下文，intro 价 $2/$10（至 2026-08-31）。preset 仍是上一代 `claude-sonnet-4-6`，**应替换或新增** |
| `claude-opus-4-7` / `claude-opus-4-6` / `claude-opus-4-5-20251101` | 🟡 低 | 仍 Active 的前代 Opus。是否列取决于产品取向（见 §3） |
| `claude-sonnet-4-5-20250929` | 🟡 低 | 仍 Active 的前代 Sonnet。同上 |

### 无需列入

- Mythos 5 / Mythos Preview：Project Glasswing 受邀，非通用 API。
- Deprecated / Retired 模型：用户调不通或不可靠。
- EU 变体：官方无此 API ID。

---

## 3. 推荐 `model_list.default` 最终清单

### 推荐方案 A：纯旗舰（4 个，最小集，与官方「Latest models comparison」对齐）

```json
["claude-opus-4-8", "claude-fable-5", "claude-sonnet-5", "claude-haiku-4-5"]
```

理由：
- 与 overview 旗舰表 1:1 对齐，覆盖 Anthropic 当前主推的 4 档（Opus / Fable / Sonnet / Haiku 旗舰）。
- `claude-sonnet-5` 取代 `claude-sonnet-4-6`：旗舰已换代，Sonnet 5 上下文从 200k 升至 1M，价格档位 ($3/$15，intro $2/$10) 与 Sonnet 4.6 同级或更低。
- Haiku 用 alias `claude-haiku-4-5` 而非日期版，符合「dateless / alias 优先」惯例，与 Opus/Fable/Sonnet 风格一致。
- 不列前代 Active 模型：preset 定位是「开箱推荐档」，不是历史博物馆；用户需要老版本可手填。

### 推荐方案 B：旗舰 + 关键前代（如需向后兼容）

```json
["claude-opus-4-8", "claude-fable-5", "claude-sonnet-5", "claude-haiku-4-5",
 "claude-sonnet-4-6", "claude-opus-4-7"]
```

理由：保留 sonnet-4-6 / opus-4-7 给依赖旧快照的客户（部分企业工作流 pin 老版本做回归对照）。

### 我的推荐：方案 A

理由：
- aidog preset 是面向新用户的「默认推荐」，不是迁移工具；旗舰 4 档足够。
- 老 Active 模型仍可由用户在 platform.extra 自行追加，preset 不必背全部。
- STATIC_MODEL_IDS（passthrough.rs:233-242）同步走方案 A。

---

## 4. 关键引用（file:line / URL）

| 事实 | 引用 |
|---|---|
| 旗舰 4 模型 + API ID + 上下文窗口 + 定价 | overview 「Latest models comparison」表 |
| `claude-sonnet-5` 是 dateless 完整 ID（非 alias） | overview 注脚：「Starting with the Claude 4.6 generation, model IDs use a dateless format that is also a pinned snapshot, not an evergreen pointer.」 |
| `claude-haiku-4-5` 是 alias，解析到 `claude-haiku-4-5-20251001` | overview 表「Claude API ID / Claude API alias」两列 |
| `claude-sonnet-4-6` 仍 Active | deprecations 「Model status」表 |
| `claude-opus-4-1-20250805` Deprecated | deprecations 「Model status」表 + 「2026-06-05」段 |
| Mythos 5 受邀 / Project Glasswing | overview 「Claude Fable 5 and Claude Mythos 5」段 |
| 无 EU 模型 ID（仅 endpoint 路由差异） | overview 注脚「global endpoints / regional endpoints」段 |
| 现 preset sonnet 用 4-6 而非 5 | `src-tauri/defaults/platform-presets.json` `protocols.anthropic.model_list.default` |
| STATIC_MODEL_IDS 同步 4 模型 | `src-tauri/src/gateway/proxy/passthrough.rs:233-242`（最近核对注释 2026-07-08） |

---

## 5. Caveats

- 官方 deprecations 「Model status」表当前**未列 Sonnet 5 / Fable 5 / Opus 4.8 之外的旗舰**（即只有 Opus 4.8 / Sonnet 4.6 / Haiku 4.5 等出现在该表），但 overview 旗舰表已确认 Sonnet 5 / Fable 5 为当前 GA 模型——两表更新节奏不同，以 overview 为准。
- `claude-haiku-4-5` alias 与 `claude-haiku-4-5-20251001` 完整 ID 行为等价；选 alias 与项目其他模型 ID（dateless 风格）保持一致更美观，但若担心 alias 解析变化可用完整 ID。preset 现状用 alias，无强改必要。
- Bedrock / Vertex AI 的模型 ID（`anthropic.claude-*` / `claude-*@yyyymmdd`）与本 preset 无关——aidog 走 Claude API 直连，用第一方 ID。
- STATIC_MODEL_IDS（passthrough.rs）注释自述「月级腐化需手工核对」，本次核对日 2026-07-08 与本研究一致；若选方案 A，passthrough.rs:234-237 同步把 `claude-sonnet-4-6` 改成 `claude-sonnet-5`。

---

# 6. 最大化清单（含快照，用户选「最大化」补遗）

- **Query**: 补全 anthropic 全部公开 API id（含历史仍可调 + 日期快照版本）
- **Date**: 2026-07-08
- **新增 Sources**:
  - https://docs.anthropic.com/en/docs/about-claude/models/overview （「Latest models comparison」+「Claude API ID / Claude API alias」双列 + pinned snapshot 注脚）
  - https://docs.anthropic.com/en/docs/about-claude/model-deprecations （「Model status」+「Deprecation history」全表 — 实际是全模型最权威清单，overview/all-models 仅列当前旗舰 4 档）

> 说明：官方 `models/all-models` 页当前**未提供比 overview 更多的模型清单**——抓取该页仅出现旗舰 4 档（fable-5 / opus-4-8 / sonnet-5 / haiku-4-5），与 overview 同表。**deprecations 页的「Model status」表才是全公开 API id 的最权威清单**（含 Active 全部前代 + Deprecated）。

## 6.1 全公开 API id 表（最大化，含快照，仍可调）

| # | API id（实际请求用） | 代次 | 状态 | 退役截止 | 快照形式 | 备注 |
|---|---|---|---|---|---|---|
| 1 | `claude-fable-5` | Fable 5 | Active（GA 旗舰） | — | dateless（4.6+ = pinned snapshot） | overview 旗舰表 |
| 2 | `claude-opus-4-8` | Opus 4.8 | Active（GA 旗舰） | Not sooner than 2027-05-28 | dateless | overview 旗舰表 |
| 3 | `claude-sonnet-5` | Sonnet 5 | Active（GA 旗舰） | — | dateless | overview 旗舰表 |
| 4 | `claude-haiku-4-5-20251001` | Haiku 4.5 | Active（GA 旗舰） | Not sooner than 2026-10-15 | **dated**（pre-4.6） | overview 表「Claude API ID」列；`claude-haiku-4-5` 为其 alias，解析到本 dated id |
| 5 | `claude-opus-4-7` | Opus 4.7 | Active（前代） | Not sooner than 2027-04-16 | dateless（4.6+） | deprecations「Model status」 |
| 6 | `claude-opus-4-6` | Opus 4.6 | Active（前代） | Not sooner than 2027-02-05 | dateless（4.6+） | deprecations「Model status」 |
| 7 | `claude-sonnet-4-6` | Sonnet 4.6 | Active（前代） | Not sooner than 2027-02-17 | dateless（4.6+） | deprecations「Model status」 |
| 8 | `claude-opus-4-5-20251101` | Opus 4.5 | Active（前代） | Not sooner than 2026-11-24 | **dated**（pre-4.6） | deprecations「Model status」；无 dateless alias 公开 |
| 9 | `claude-sonnet-4-5-20250929` | Sonnet 4.5 | Active（前代） | Not sooner than 2026-09-29 | **dated**（pre-4.6） | deprecations「Model status」；无 dateless alias 公开 |
| 10 | `claude-opus-4-1-20250805` | Opus 4.1 | **Deprecated**（2026-06-05 起） | **2026-08-05**（≈1 月后下线） | dated | deprecations「Deprecation history」；仍在可调用窗口内 |

> **快照语义**（引自 overview 注脚）：
> - 4.6 代起（含 Opus 4.6/4.7/4.8、Sonnet 4.6/5、Fable 5）：dateless ID 本身就是 pinned snapshot（不是 evergreen pointer），**没有独立的日期后缀版本**——`claude-opus-4-8` 就是快照，不会再有 `claude-opus-4-8-20YYMMDD`。
> - 4.6 代前（Opus 4.5、Sonnet 4.5、Haiku 4.5、Opus 4.1）：API id 必带日期后缀（`-YYYYMMDD`）才是快照真值；dateless 形式（如 `claude-haiku-4-5`）是 alias 指针。
> - 故「最大化含快照」对 4.6+ 模型就是 dateless id 本身；对 pre-4.6 模型必须用 dated id。

## 6.2 明确排除的 id（理由附）

| 排除项 | 理由 |
|---|---|
| `claude-mythos-5` / `claude-mythos-preview` | Project Glasswing 受邀 only，非自服务 API；普通 key 调不通 |
| `claude-sonnet-4-20250514` / `claude-opus-4-20250514` | Retired 2026-06-15，不可调 |
| `claude-3-7-sonnet-20250219` | Retired 2026-02-19 |
| `claude-3-5-sonnet-20240620` / `-20241022` | Retired 2025-10-28 |
| `claude-3-5-haiku-20241022` | Retired 2026-02-19 |
| `claude-3-opus-20240229` | Retired 2026-01-05 |
| `claude-3-haiku-20240307` | Retired 2026-04-20 |
| `claude-3-sonnet-20240229` / `claude-2.x` / `claude-1.x` / `claude-instant-*` | Retired 2024-11 ~ 2025-07 |
| `anthropic.claude-*-v1:0` 等 Bedrock / `claude-*@YYYYMMDD` Vertex AI 变体 | aidog 走 Claude API 直连，非云市场路由 id |
| `claude-haiku-4-5-20251001-v1` | 这不是 Claude API id，仅出现在 overview 「AWS Bedrock ID」列（`anthropic.claude-haiku-4-5-20251001-v1:0`），是 Bedrock 平台专用后缀，Claude API 不识别 |

## 6.3 推荐 `model_list.default` 最大化清单

### 推荐方案 C：最大化（10 个 id，含仍 Active 前代 + Haiku 4.5 alias）

```json
[
  "claude-fable-5",
  "claude-opus-4-8",
  "claude-sonnet-5",
  "claude-haiku-4-5",
  "claude-opus-4-7",
  "claude-opus-4-6",
  "claude-sonnet-4-6",
  "claude-opus-4-5-20251101",
  "claude-sonnet-4-5-20250929",
  "claude-opus-4-1-20250805"
]
```

### 关键决策：alias 与 dated 快照是否都列？

**建议：只列一个，不重复。** 理由：

1. **4.6+ 模型无重复问题**——dateless id 就是 pinned snapshot，没有 dated 版本可列。
2. **pre-4.6 模型（Opus 4.5 / Sonnet 4.5 / Haiku 4.5 / Opus 4.1）**只有 dated 是真值；dateless（如 `claude-haiku-4-5`）是 alias。
   - Haiku 4.5：官方 overview 表「Claude API alias」列明确给出 `claude-haiku-4-5`，alias 稳定。**推荐用 alias `claude-haiku-4-5`**（与 4.6+ dateless 风格一致，UI 显示更整洁，官方文档同时背书）。
   - Opus 4.5 / Sonnet 4.5 / Opus 4.1：**官方未公开 dateless alias**，必须用 dated id（`claude-opus-4-5-20251101` 等），无法 alias 化。
3. **同时列 alias 与 dated 的坏处**：UI 会显示两条同义条目（如 `claude-haiku-4-5` 和 `claude-haiku-4-5-20251001`），用户误以为不同模型，选择困难；且 Anthropic 文档明确两者解析等价。

**故方案 C 中 Haiku 4.5 仅列 alias `claude-haiku-4-5`，不重复列 dated `-20251001`。**

### 是否纳入 `claude-opus-4-1-20250805`（Deprecated）？

**建议纳入但标注**。理由：
- 退役日 2026-08-05，当前（2026-07-08）仍在可调用窗口（≈1 个月）。
- 「最大化含快照」字面承诺全覆盖；用户若有遗留工作流仍依赖该 id，能选到比不能选到更安全。
- 风险：1 个月后下线，届时 preset 出现失效项。**需在 release notes / 注释里标「deprecated，2026-08-05 后移除」**。
- 替代方案：若担心短期维护成本，可省略此项得 9-id 清单（仍覆盖全部 Active）。

### 排序建议

旗舰 4 档置顶（Fable / Opus / Sonnet / Haiku，与 overview 旗舰表同序）→ Opus 前代倒序（4-7 → 4-6 → 4-5）→ Sonnet 前代（4-6 → 4-5）→ Deprecated 末尾（Opus 4.1）。

## 6.4 引用映射（每 id → 官方文档位置）

| id | 出处 |
|---|---|
| `claude-fable-5` | overview「Latest models comparison」表 + pinned snapshot 注脚 |
| `claude-opus-4-8` | overview 旗舰表 + deprecations「Model status」Active 行（退役日 2027-05-28） |
| `claude-sonnet-5` | overview 旗舰表 + pinned snapshot 注脚（4.6+ dateless） |
| `claude-haiku-4-5-20251001` | overview 旗舰表「Claude API ID」列；deprecations「Model status」Active（2026-10-15）|
| `claude-haiku-4-5`（alias）| overview 旗舰表「Claude API alias」列 |
| `claude-opus-4-7` | deprecations「Model status」Active（退役日 2027-04-16） |
| `claude-opus-4-6` | deprecations「Model status」Active（退役日 2027-02-05） |
| `claude-sonnet-4-6` | deprecations「Model status」Active（退役日 2027-02-17） |
| `claude-opus-4-5-20251101` | deprecations「Model status」Active（退役日 2026-11-24） |
| `claude-sonnet-4-5-20250929` | deprecations「Model status」Active（退役日 2026-09-29） |
| `claude-opus-4-1-20250805` | deprecations「Deprecation history」Deprecated 2026-06-05，retire 2026-08-05 |
| snapshot 语义（4.6+ dateless = pinned） | overview 注脚：「Starting with the Claude 4.6 generation, model IDs use a dateless format that is also a pinned snapshot, not an evergreen pointer.」 |
| pre-4.6 alias → dated 解析 | overview 注脚：「For models before the 4.6 generation, entries in the Claude API alias column are convenience pointers that resolve to a dated model ID.」 |

## 6.5 Caveats（最大化方案特有）

- **all-models 页等同 overview**：抓取 `models/all-models` 仅得旗舰 4 档，**没有更全的历史清单**。如官方未来在该页扩展，本研究需重核。
- **Opus 4.1 即将下线**：纳入 最大化清单 后，2026-08-05 后 preset 会出现失效项，需排期清理或在 UI 标灰。
- **Opus 4.5 / Sonnet 4.5 无 alias**：官方未公开 dateless alias，preset 必须带日期后缀，与 4.6+ dateless 风格不齐——这是 Anthropic 命名遗留，无法规避。
- **Haiku 4.5 alias vs dated 取舍**：方案 C 选 alias 与 4.6+ 风格一致；若担心 alias 解析语义未来变更（官方目前明确不会），可改用 `claude-haiku-4-5-20251001` 显式快照——二选一，不重复列。
- **STATIC_MODEL_IDS 同步**：`src-tauri/src/gateway/proxy/passthrough.rs:233-242` 的静态模型 id 表也应同步方案 C（或至少方案 A 旗舰 4 档），避免 `GET /models` 与 preset 不一致。
