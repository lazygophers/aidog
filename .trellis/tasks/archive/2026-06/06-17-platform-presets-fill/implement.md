# Implementation Plan — 补齐所有平台预设

## 执行编排（资源互斥：getDefaultEndpoints/getDefaultModels 同住 Platforms.tsx，禁并行编辑）

```mermaid
graph TD
  subgraph R[Phase R: 并行只读研究 (trellis-research, 无 worktree)]
    R1[R1 一方平台组<br/>官方+国内官方]
    R2[R2 聚合平台组]
    R3[R3 第三方/中转组]
  end
  subgraph E[Phase E: 单一编辑 agent (worktree 隔离)]
    E1[E1 据 3 研究文件改 Platforms.tsx<br/>串行落盘 + yarn build]
  end
  subgraph C[Phase C: 质检]
    C1[trellis-check]
  end
  R1 --> E1
  R2 --> E1
  R3 --> E1
  E1 --> C1
```

## Phase R — 并行研究（3 个 trellis-research，read-only，各写 research/*.md）

每 agent：逐平台 WebSearch 官方文档，核 **base_url（含版本前缀，禁额外拼接）** 与 **当前推荐默认模型 API id**，标核查日期 + 来源 URL。无引用结论标「推测:」。

- **R1 一方平台组** → `research/presets-firstparty.md`
  平台：anthropic, openai, codex, gemini, glm, glm_en, kimi, minimax, minimax_en, bailian, bailian_coding, deepseek, stepfun, stepfun_en, doubao, doubao_seed, byteplus, qianfan, **xiaomi_mimo**, bailing, longcat
  重点：①校正已有 base_url/模型是否过时 ②补默认模型空缺 ③**xiaomi_mimo 补 openai 端点**（token-plan host：`token-plan-cn.xiaomimimo.com/v1`，按量 host：`api.xiaomimimo.com/v1`，二者取舍见 xiaomi research §4.4）+ 默认模型（MiMo 系列）
- **R2 聚合平台组** → `research/presets-aggregator.md`
  平台：openrouter, siliconflow, siliconflow_en, aihubmix, dmxapi, modelscope, shengsuanyun, atlascloud, novita, therouter, cherryin, nvidia
  重点：base_url 校正；聚合平台多模型，**默认模型仅在官方有明确推荐时填**，否则标「N/A（多模型，留空）」
- **R3 第三方/中转组** → `research/presets-thirdparty.md`
  平台：packycode, cubence, aigocode, rightcode, aicodemirror, pateway, ccsub, apikeyfun, apinebula, sudocode, claudeapi, claudecn, runapi, relaxycode, crazyrouter, sssaicode, compshare, compshare_coding, micu, ctok, eflowcode, lemondata, pipellm, opencode, newapi, claude_code
  重点：多为 claude-code 代理透传，base_url 校正为主；默认模型多继承 anthropic，**仅平台官方有自有默认模型时填**

每文件输出表：`平台 | 现有 base_url | 核实后 base_url(变更?) | 现有默认模型 | 核实后默认模型 | 来源URL | 核查日期`。

## Phase E — 编辑（单 agent，worktree）

读 3 研究文件 → 改 `src/pages/Platforms.tsx`：
1. `getDefaultEndpoints`：xiaomi_mimo 补 openai 端点；应用所有 base_url 校正。
2. `getDefaultModels`：填补默认模型空缺（仅研究确认有官方默认值的平台），保留 `// 截至 <日期> 核对官方` 注释 + 过时提醒。
3. 不破坏 url-construction-rule（base_url 含版本前缀，禁额外拼接）。
4. `yarn build` 绿。如涉 Protocol 枚举变动 → models.rs + api.ts 双写 + cargo build/clippy（预期不涉，全是已有变体）。

## Phase E — 最终改动集（research 三组核实后锁定，仅此 3 处）

> 研究结论：~60 平台预设绝大多数已最新。base_url 0 处确证需改（第三方 2 处「推测:」、聚合 3 处未 100% 证实路径——均保持原值）。实际改动：

1. **getDefaultModels — minimax / minimax_en**：`MiniMax-M2.7` → `MiniMax-M3`（2026-06-02 新旗舰，HF+OpenRouter 双源确认，见 research/presets-firstparty.md）。
2. **getDefaultModels — glm / glm_en**：`glm-4.6` → `glm-5.2`（前瞻跟官方新旗舰；同步更新 `:382` 弃用注释为「glm-4.6 已被 glm-5.2 取代，coding plan 端如遇不兼容回退 4.6」）。
3. **getDefaultEndpoints — xiaomi_mimo**：现仅 anthropic 端点，**补 openai 端点**：
   ```ts
   { protocol: "openai", base_url: "https://api.xiaomimimo.com/v1", client_type: "codex_tui" },
   ```
   并 **getDefaultModels 新增 xiaomi_mimo**：`{ default: "mimo-v2.5-pro" }`（见 research/presets-firstparty.md xiaomi 节）。

其余平台（含 qianfan/longcat/stepfun/doubao 等 coding 透传无模型槽位、聚合多模型）**保持原值/留空**——研究已核实，不强改。

## Phase C — 质检

trellis-check：yarn build + check:i18n（如涉文案）；预设值有来源引用；无空 base_url。

## 失败处理

- research 查不到官方 → 该平台标「未找到，保持原值」，不编造。
- 编辑 build 失败 → 读错误定点修，≤2 轮，仍败 STOP 回传。
