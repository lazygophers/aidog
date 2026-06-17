# 默认模型预设（getDefaultModels）

加平台时给该平台预填默认模型槽位，让用户选完平台后表单自动带上主力型号、列表卡片也能在「未配置 + 无上游可用模型」时回退展示预设模型。file:line 校对于 2026-06-15（child A 已落地，下述函数名/字段为真实实现）。

---

## 0. 机制概览

预设住前端 `src/pages/Platforms.tsx`，与 `getDefaultEndpoints` 并列：

| 角色 | 符号 | file:line |
|---|---|---|
| 默认模型预设函数 | `getDefaultModels(protocol, codingPlan?)` | `src/pages/Platforms.tsx:371` |
| 表单 auto-fill 调用点 | `onProtocolChange` 内 | `src/pages/Platforms.tsx:1620` |
| 列表卡片回退展示调用点 | `configuredModels` IIFE | `src/pages/Platforms.tsx:1164` |
| 槽位结构（Rust） | `PlatformModels` | `src-tauri/src/gateway/models.rs:235` |
| 槽位结构（TS） | `PlatformModels` / `ModelSlot` | `src/services/api.ts:56-62` / `:47` |
| 路由消费 | `resolve_model` | `src-tauri/src/gateway/router.rs:317` |

---

## 1. `getDefaultModels` 签名与现状

```ts
// Platforms.tsx:371（模型名截至 2026-06 核对官方发布说明，迭代月级，过时由 fetchModels 覆盖）
function getDefaultModels(protocol: Protocol, codingPlan?: boolean): Partial<Record<ModelSlot, string>> {
  const cp = !!codingPlan;
  const presets: Partial<Record<Protocol, Partial<Record<ModelSlot, string>>>> = {
    // ── 官方 ──
    anthropic: { default: "claude-opus-4-8", opus: "claude-opus-4-8", sonnet: "claude-sonnet-4-6", haiku: "claude-haiku-4-6" },
    openai:    { gpt: "gpt-5.5" },
    codex:     { gpt: "gpt-5.5-codex" },
    // gemini: 槽位语义不匹配，留空
    // ── 国内官方 ──
    glm:        { default: "glm-4.6" },
    glm_en:     { default: "glm-4.6" },
    kimi:       { default: cp ? "kimi-k2.7-code" : "kimi-k2.6" },  // coding plan 切型号
    minimax:    { default: "MiniMax-M2.7" },
    minimax_en: { default: "MiniMax-M2.7" },
    bailian:    { default: "qwen3.7-max" },
    deepseek:   { default: "deepseek-v4-flash" },
  };
  return { ...(presets[protocol] || {}) };
}
```
> 上方为节选；实际源码每行带弃用时间注释（如 glm-4.6 将 2026-07-09 弃用）。模型名是动态值，**核对以源码为准**，本文档不追逐每次型号更新。

特征：
- 返回 `Partial<Record<ModelSlot, string>>`（**Partial，非 exhaustive**）——未覆盖平台返回 `{}`，不报错，表单保持空。
- **单个 model 名/槽位**（非列表），如 `default: "glm-4.6"`。
- coding plan 平台用 `cp` 三元切型号（kimi 示例）。
- 只填预设有值的槽位，其余空。

---

## 2. 加新平台时怎么填

在 `presets` map 里加一行该平台 → 槽位对象：

```ts
foo: { default: "foo-model-v1" },          // OpenAI 兼容平台多归 default 槽
// 或多槽位：
bar: { opus: "bar-large", sonnet: "bar-mid", haiku: "bar-small" },
```

槽位选择对齐 `resolve_model`（`router.rs:317`）匹配规则：请求模型名（小写）含 `opus`/`sonnet`/`haiku`/`gpt` → 用对应槽位；否则用 `default`；无 `default` → 透传（去 `[budget]` 后缀）。

填法准则（参照现有注释 `Platforms.tsx:370`「不确定的不硬填，避免过时/编造」）：
- **取该平台当前主力型号**，确定才填；不确定留空（让用户手填或点「拉取模型」）。
- OpenAI Chat Completions 兼容的国内平台通常只填 `default`（无 anthropic 槽位语义）。
- gemini 类槽位语义不匹配（无 opus/sonnet/gpt 对应）→ 留空。

---

## 3. 两个消费点（加平台无需改，已泛化）

### ① 表单 auto-fill — `Platforms.tsx:1620`
切协议时 `getDefaultModels(newProtocol, cp)` → 展开进 `setModels`：
```ts
const defaultModels = getDefaultModels(newProtocol, cp);
setModels({ default: "", sonnet: "", opus: "", haiku: "", gpt: "", ...defaultModels });
```
即「全空底 + 预设覆盖」，预设没覆盖的槽位保持空。

### ② 列表卡片回退展示 — `Platforms.tsx:1164`
卡片展示模型优先级：已配置 `p.models` → 上游 `available_models` → **预设 `getDefaultModels` 回退**：
```ts
if (explicit.length > 0) return explicit;
if ((p.available_models?.length ?? 0) > 0) return explicit;
return allModelValues(getDefaultModels(p.platform_type, hasCodingEndpoint));
```
`hasCodingEndpoint` 从 endpoints 推（`(p.endpoints ?? []).some(ep => ep.coding_plan)`，`:1159`）。

> 两个调用点都按 `Protocol` 泛化，**加平台只改 §2 的 presets map 一处**，消费点无需碰。

---

## 4. 与槽位结构 / 路由的契约

- **Rust `PlatformModels`**（`models.rs:235`）：`default/sonnet/opus/haiku/gpt`，全 `Option<String>` 默认 None。
- **TS `PlatformModels`**（`api.ts:56-62`）+ `ModelSlot`（`api.ts:47`）同名。
- 预设填的 key 必须是 `ModelSlot` 之一（`default`/`sonnet`/`opus`/`haiku`/`gpt`），否则 TS 报错（`Record<ModelSlot, …>` 约束）。
- 后端 `create_platform`（`db.rs:508`）原样落 `models`，对默认值零知识——预设的物化全在前端 setModels。

---

## 5. 与 fetchModels 自动归类的关系

预设是「选平台即填」；用户也可点「拉取模型」按钮（`fetchModels` → `autoCategorize`，`Platforms.tsx:596`）从上游 `/models` 拉列表后按正则归槽（opus/sonnet/haiku 正则、gpt 非 mini、首个未分配兜底进 default）。二者独立：预设给静态主力型号，fetchModels 给实时全量。加平台填预设**不影响** fetchModels 行为。
