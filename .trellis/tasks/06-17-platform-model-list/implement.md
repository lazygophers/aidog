# Implementation Plan — 平台内置模型列表供下拉选择

## 编排（资源互斥：getDefaultModelList 数据 + 编辑器都在 Platforms.tsx，禁并行编辑）

```mermaid
graph TD
  subgraph R[Phase R: 并行研究 (trellis-research, 无 worktree)]
    R1[R1 一方平台 模型列表]
    R2[R2 聚合平台 代表模型]
    R3[R3 第三方/中转 模型来源]
  end
  E1[Phase E: 单一编辑 agent worktree<br/>getDefaultModelList + 编辑器 dropdownSource]
  C1[Phase C: trellis-check]
  R1 --> E1
  R2 --> E1
  R3 --> E1
  E1 --> C1
```

## Phase R — 并行研究（3 组，逐平台 WebSearch 核官方当前模型列表，写 research/*.md）

输出每平台：`平台key | 候选模型 API id 列表(有序,旗舰在前) | 来源URL | 核查日期`。无来源标「推测:」；查不到标「未找到，留空靠 fetchModels」。

- **R1 一方平台** → `research/models-firstparty.md`
  anthropic, openai, codex, gemini, glm, glm_en, kimi, minimax, minimax_en, bailian, bailian_coding, deepseek, stepfun, stepfun_en, doubao, doubao_seed, byteplus, qianfan, xiaomi_mimo, bailing, longcat
  给官方在售模型 API id 全列表（chat/coding 相关）。
- **R2 聚合平台** → `research/models-aggregator.md`
  openrouter, siliconflow, siliconflow_en, aihubmix, dmxapi, modelscope, shengsuanyun, atlascloud, novita, therouter, cherryin, nvidia
  多模型聚合 → 给「常用代表模型」子集（5-15 个热门，如 claude/gpt/deepseek/qwen 旗舰），注明 fetchModels 为主源。
- **R3 第三方/中转** → `research/models-thirdparty.md`
  packycode, cubence, aigocode, rightcode, aicodemirror, pateway, ccsub, apikeyfun, apinebula, sudocode, claudeapi, claudecn, runapi, relaxycode, crazyrouter, sssaicode, compshare, compshare_coding, micu, ctok, eflowcode, lemondata, pipellm, opencode, newapi, claude_code
  多为 claude-code 代理 → 候选列表 = 当前 Claude 旗舰系列（claude-opus/sonnet/haiku 最新）；有自有模型的列其自有。

## Phase E — 编辑（单 agent，worktree）

1. 新增 `getDefaultModelList(protocol: Protocol, codingPlan?: boolean): string[]`（紧邻 getDefaultModels），数据来自 3 研究文件；每平台候选列表，有序，旗舰在前。带 `// 截至 <日期> 核对官方` 注释。
2. 编辑器（`:2412-2458` 区段）：把 `availableModels.length > 0 ? availableModels : []` 改为 `availableModels.length > 0 ? availableModels : getDefaultModelList(protocol, codingPlan)`，使 ▾ 下拉与 filtered 在未刷新时也用内置列表；▾ 按钮可见性同步（`availableModels.length>0 || builtinList.length>0`）。
3. 保留自由输入（input 仍可手输），fetchModels 成功仍覆盖为 available_models。
4. `getDefaultModels`（单值，初始填充）保留；可令 default 槽 = 列表[0]（如已有单值则不动）。
5. **附带纠错（R3 研究发现）**：`getDefaultModels` anthropic 槽 `haiku: "claude-haiku-4-6"` 改为 `claude-haiku-4-5`（Anthropic 无 Haiku 4.6，最新 4-5，官方 overview + claude-api skill + LiteLLM 三源确认）。
6. `yarn build` 绿。

## Phase C — 质检

trellis-check：build 绿；选无 available_models 的平台 → 槽位 ▾ 下拉列内置候选；fetchModels 后切 available_models；git status 仅 Platforms.tsx；列表值有来源。

## 失败处理

- research 查不到某平台官方列表 → 留空（靠 fetchModels），不编造。
- 编辑 build 失败 → 定点修 ≤2 轮，仍败 STOP 回传。
