# PRD — 阶跃补默认模型 + 全平台模型配置一致性审计

## 背景
用户报「阶跃星辰缺少默认的模型配置」。根因:`getDefaultModels`(Platforms.tsx:456)preset 对象**无 stepfun/stepfun_en 条目** → 添加阶跃平台时默认模型字段空(下拉候选 lists:526 有 `step-3.7-flash/step-3.5-flash`,但默认值空)。顺带审计所有平台模型配置点确保存在 + 一致 + 沿用既有 research(2026-06-17)基线对齐"最新"。

## 决策(用户已锁)
| 维度 | 决策 |
|---|---|
| 深度 | 补漏 + 一致性核对(不重新 research,沿用 archive/2026-06/06-17-platform-model-list/research/*.md) |
| 范围 | 4 配置点全核:getDefaultModels presets / lists 候选 / STATIC_MODEL_IDS / protocol.rs 枚举 |
| 时间线 | 项目设定 2026-07,模型版本以 research(2026-06-17 核查)为准,不追真实世界当前 |

## 已知差集(planning 已 grep 实证,implement 逐项核 + 补)
### getDefaultModels(Platforms.tsx:456-483)漏 default 的平台
- `stepfun`: 补 `{ default: "step-3.7-flash" }`(research:94 静态确认旗舰)
- `stepfun_en`: 补 `{ default: "step-3.7-flash" }`(research:100 同 stepfun)
- `byteplus`: 补 `{ default: "doubao-seed-2-0-pro" }`(research:106 国际 doubao 旗舰;lists:531 首项一致)
- `doubao_seed`: 补 `{ default: "doubao-seed-2-0-pro" }`(research:111 同 doubao 体系)

### lists(Platforms.tsx:508)漏候选的平台
- `opencode_zen`: getDefaultModels:480 有 `default: "big-pickle"`,lists 漏候选 → 补 `opencode_zen: ["big-pickle", ...fetchModels 兜底]`(research/models-aggregator.md 核)

### 首项一致性(lists[0] 应 = getDefaultModels default)
- `bailian`: getDefaultModels:472 `default: "qwen3.7-max"`(无 cp 分支);lists:521 cp 时首项 `qwen3-coder-plus`。**getDefaultModels bailian 应加 cp 分支**:`{ default: cp ? "qimi3-coder-plus" : "qwen3.7-max" }`(对齐 lists cp 首项 + kimi/deepseek cp 模式)

### STATIC_MODEL_IDS(passthrough.rs:229)
- 设计:仅 Claude+Codex 官方默认 5 个(`/models` 端点 tokenless 探测用,CLAUDE.md 明确不依赖 group)。当前:`claude-opus-4-8/sonnet-4-6/haiku-4-5/gpt-5.5-codex/gpt-5.5`
- 核对:`haiku-4-5` vs research:20 anthropic 候选 `haiku-4-6` —— 确认当前 haiku 旗舰版本(lists:510 亦 haiku-4-5,以 lists/research 一致为准,不擅自升)

### protocol.rs 枚举(models/protocol.rs,100 变体)
- 核对 getDefaultModels/lists 的平台 key(stepfun/byteplus/doubao_seed/opencode_zen 等)在 Protocol 枚举存在(防 typo 致路由失败);若缺属另一类 bug,标 `需要:` 回传

## 交付
1. **Platforms.tsx `getDefaultModels`**(:456-483)—— 补 stepfun/stepfun_en/byteplus/doubao_seed default;bailian 加 cp 分支
2. **Platforms.tsx `lists`**(:508+)—— 补 opencode_zen 候选;逐平台核对 lists[0] = getDefaultModels default(首项锚定 route resolve)
3. **passthrough.rs `STATIC_MODEL_IDS`**(:229)—— 核对 5 个版本是否当前 Claude/Codex 旗舰(对齐 lists/getDefaultModels 的 anthropic/openai/codex 首项);若 haiku 版本不一,以三者一致为准修
4. **protocol.rs**(:Protocol 枚举)—— 验证平台 key 存在(只读核对,预期无改动;若缺标 `需要:`)
5. **i18n / 测试** —— 模型名变更若涉及 i18n key 跟随;platformPaste.test.ts 等若断言旧模型名同步

## 验收
- 阶跃平台添加时默认模型字段 = `step-3.7-flash`(非空)
- getDefaultModels 所有条目的 default ∈ 对应 lists 候选 且 = lists[0](首项一致)
- getDefaultModels 覆盖 lists 中所有"有候选"的一方/三方平台(stepfun/stepfun_en/byteplus/doubao_seed/opencode_izen 等;聚合平台 fetchModels 为主源可不填 default,research 注明除外)
- STATIC_MODEL_IDS 5 个版本与 lists/getDefaultModels 的 anthropic/openai/codex 首项一致
- protocol.rs 枚举含全部 getDefaultModels/lists 平台 key
- `yarn build` + `cargo test`/`clippy` + `scripts/check-i18n.mjs` 全绿

## 非目标(YAGNI)
- 重新 research 各平台 2026-06-30 最新模型(决策=沿用 2026-06-17 research)
- 聚合平台(openrouter/aihubmix/dmxapi/...)default 填充(它们 fetchModels 为主源,research 注冷启动占位,不填 default 合理)
- fetchModels 运行时拉取逻辑改动(本 task 仅静态配置)
- 定价(price_sync)刷新(独立 task)

## 调度
单 task,write-files 跨 `src/pages/Platforms.tsx` + `src-tauri/src/gateway/proxy/passthrough.rs` + `src-tauri/src/gateway/models/protocol.rs`(只读核)。文件集与 active task(deeplink 改 Platforms/ShareModal、paste-base64 改 platformPaste.ts)有 Platforms.tsx 相交 → **与 deeplink/paste-base64 串行**(同文件)。

```
deeplink(改 Platforms.tsx) → 本 task(改 Platforms.tsx) 串行
paste-base64(改 platformPaste.ts) → 不相交可并行
```

active 满 2 时排队,腾槽后 start。

## 风险
- bailian cp 分支改动波及 coding plan 平台路由(coding plan 默认模型变)——implement 后 cargo test(router/usage_color)回归
- STATIC_MODEL_IDS 改版本致 test_passthrough:226/245 长度断言 —— 同步改测试
- 模型名月级腐化(CLAUDE.md 注),本 task 锁 2026-06-17 基线,不追最新
