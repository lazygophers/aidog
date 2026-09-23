# Registry 数据持续对齐

## Goal

`src-tauri/defaults/registry/` 模型条目数据无缺失：分层口径（2026-09-23 拍板）——
`family` / `version` / `capabilities` / `context_window` 四字段凡官方有公布即必补清零；
`thinking_supported` / `thinking_toggleable` / `predecessor` / `display_name` 尽力补。

## Trigger

连续执行：用户 2026-09-23 令「现在开始跑，一直跑到所有东西都没问题」。
按平台分批推进，每轮 1-2 个平台（约 50-200 条），轮间无间隔直到门禁达标。

## Loop body（每轮）

1. 选平台：优先用户报过的，其余按必补字段缺失量降序。
2. research agent 查官方一手来源（平台定价页 / 模型卡 / docs，jina reader 破地区封锁），
   产出「字段值 + 来源 URL」事实表。
3. 依据事实表文本级编辑 registry JSON（禁 JSON round-trip 全文件重写，禁从 canonical
   推断 family/version，数值字面保真）。
4. `node scripts/bump-registry-last-updated.mjs` 盖戳。
5. `yarn check:registry` + `cargo test --manifest-path src-tauri/Cargo.toml -p aidog_db` 全绿。
6. commit（路径限定 platforms/<code>/ 与 index.json）。

## Checkpoint

无人工 checkpoint（用户 2026-09-23 授权 agent 自动写入，事后审计）。
写入质量铁律不因自动写入放宽：只写官方核验值，缺省=未知。

## Gate（验收判据）

覆盖率硬阈值：四必补字段各自覆盖率 ≥90%（起线，随轮次单调收紧）。判定命令：

```bash
AIDOG_REGISTRY_COVERAGE_MIN=90 node scripts/check-registry.mjs
```

默认不带 env 只报数不拦截；thinking_*/predecessor 只统计不设门。
门禁全绿 = 循环终止。

## State

- 2026-09-23 基线（4570 条，check-registry ⑩ 实测）：family 15.8% / version 5.3% /
  capabilities 96.6% / context_window 72.5%；thinking_supported 20.9% /
  thinking_toggleable 18.1% / predecessor 2.8% / display_name 9.5%。
- 平台缺失排名头部：bailian 1016 / bailian_coding 1014 / aihubmix 906 /
  openrouter 744 / bailian_en 732 / therouter 534。
- 2026-09-23 门禁基建提交 `011c60b`（check-registry ⑩ 覆盖率统计 + env 门禁）。
- 2026-09-24 第 1 轮 glm `1c0f5ba`：官方冲突修正 6 处（autoglm-phone 20K、
  glm-4.1v-thinking-flash 64K/16K、flashx 输出 16K、glm-4.7-flash 输出 128K、
  glm-4v-flash 16K/1K）+ glm-4.6v thinking 两字段 + glm-4-32b-0414-128k 展示名
  + 新增 glm-5.3-flashx。glm 平台 family/version 官方不公布 → 合法缺省。
  未采纳：AutoGLM-Phone-Multilingual（限时免费无稳定价、上下文未公布，攒待问）；
  glm-4.5-flash 两站冲突（z.ai 200K vs bigmodel 128K）保持 bigmodel 口径 131072。
- 2026-09-24 第 2 轮 bailian `589203e`：32 文件窗口/输出硬伤修正（kimi-k3 1M、
  deepseek 系、glm 系拿错列、MiniMax-M3 1M、qwen 系 max_in 精确值）+ 视觉能力补
  （kimi-k3 / MiniMax-M3 / kimi-k2.7-code）+ thinking_toggleable 补（deepseek-v3.2、
  qwen-turbo、qwen-plus 系）+ 2 处旧价修正（qwen3.7-max、qwen-plus-2025-07-28）。
  b 说明：bailian_coding 价格/上下文可平移，bailian_en / bailian_coding_en 价格
  不可平移（国际区倍率不统一），需单独取证 alibabacloud.com。
- 第 3 轮候选（bailian 深挖）：分层价格不完整的旧价（qwen3.7-plus/flash、qwen3.6
  系、qwen3-max）、重复条目（qwen3.5-plus-02-15 ↔ 2026-02-15 等双条目）、
  deepseek-v4-pro/flash 分时计价缺 time_tiers、新模型（deepseek-v4.1-flash 分时价、
  qwen3.8-omni 系等）、可疑下线条目复核（qwen3.6-27b 等 6 条）。
- 2026-09-24 第 3 轮 bailian `acc68e8`（bailian_coding 镜像）：5 个模型分层价全量
  重写（qwen3.7-plus/flash、qwen3.6-plus/flash、qwen3-max，档界按官方 256k=262145）；
  分时计价落平台级 peak 窗口（北京 8-22 ×2，精确限定 deepseek-v4-pro-0813 /
  v4-flash-0731 / v4.1-flash，base 价=闲时）——registry 的 time_tiers 是生效日期档
  不是峰谷窗口，勿混用；新模型 3 个（deepseek-v4.1-flash、qwen3.7-text-embedding-flash、
  qwen3.7-text-rerank）；非官方 id 清理（删 qwen3.5-plus-02-15 / flash-02-23，
  20260420 改名 2026-04-20）；下线复核 6 条全在售并修正其窗口/价格/视觉能力。
- 遗留：qwen3.8-omni-flash 价格官方模型页缺失（需计费文档）；qwen3.8-omni-flash-realtime
  音频/文本双价 registry 单价结构装不下；qwen3.5-plus-2026-04-20 快照价与主版本不同源未核。
- 2026-09-24 第 4 轮 aihubmix `a942158`（420 文件，官方 Models API 全量对齐）：
  554 条目修正（ctx/maxout 208 处、价格 50 处、填充 display_name/context/maxout/
  thinking/cache 价 643 处）；删 26 条官方已下架（含裸 glm-5、全部 *-free 变体）；
  新增 15 个官方在售新模型（claude-opus-5-5、gpt-6-luna/sol、grok-4.7、
  deepseek-v4.1-flash、glm-5.3-flashx、mimo-v2.6 系等）。新条目 official:false——
  aihubmix 是聚合站，官方渠道语义由 canonical 关联其它平台条目承载（测试不变量）。
  alicloud-glm-5 因官方未公布 ctx 无法过 schema，未登记。
- 待确认攒问：① aihubmix 官方在售但 registry 未收的 400+ 条（旧代 gpt-3.5/4、
  embedding/image/video 类）要不要全量镜像；② 官方 discount 限时优惠与
  context_tiers 分档要不要进 registry（现无此维度）。
- 2026-09-24 第 5 轮 openrouter `c23dacd`（287 文件，官方 models API 全量对齐）：
  299 条目修正（价格 103 系含 qwen/z-ai/deepseek 大面积旧价、窗口三连错）；删 33 条
  官方下架（含 claude-opus-4、:batch/:free 变体、~openai/gpt-latest alias）；
  新增 21 个（claude-opus-5.5、gpt-6 四系、grok-4.7、deepseek-v4.1-flash、
  glm-5.3-flashx、mimo-v2.6 三档等）。openrouter/* 元路由 5 条官方价为负（返点）
  schema 不收，保持原值。NO_OFFICIAL_CHANNEL +2（anthropic/claude-opus-4、
  kwaipilot/kat-coder-pro-v2：官方渠道已下架仅剩镜像）。
- 2026-09-24 第 6 轮 bailian 国际区 `d3765ce0` + `5600050`：Singapore 价对齐（kimi-k3、
  qwen3.6-max-preview 分档）、deepseek 闲时 base + 平台 peak ×2 窗口、ctx 修正、
  thinking 填充 29 条。GLM/Kimi 国际价 `[reader]` 不可复核、MiniMax 国际在售性
  存疑——均未写。另 `a2cc505` 补第 1 轮漏提交的 glm-5.3-flashx.json（pathspec
  commit 不带 untracked，新文件必须先 git add——教训已犯两次）。
- 2026-09-24 第 7 轮 therouter `2b659e2`：9 价格修正（gpt-5.6-luna 旧价恰 5 倍）、
  9 新模型（claude-fable-5-1、gemini-3.7/3.8-flash、gpt-6-astra、qwen3.8×3、
  zai glm-5.3×2——zai/ 前缀 3 条 official:true 对齐 therouter 惯例）、6 下架。
  遗留：gpt-image-2.5-flare/sunburst（per-image 计价）未登记；therouter 264 个
  详情页的 thinking 逐页核对未做。
- 待确认攒问：① aihubmix 官方在售但 registry 未收的 400+ 条要不要全量镜像；
  ② 官方 discount 限时优惠与 context_tiers 分档要不要进 registry；③ bailian_en
  GLM/Kimi 国际价复核（需国际站计价页明文）；④ therouter per-image 计价模型
  登记口径；⑤ anthropic/google 官方平台自身条目未在本循环核过（fable-5-1 官方
  渠道缺口即来自这里）。
- DB 侧独立线：过期行清理（prune_model_entries）已提交 `95808e3`（门禁：aidog_db
  351 passed / aidog_core desktop 全过 / clippy -D warnings 零 warning，2026-09-24）。
