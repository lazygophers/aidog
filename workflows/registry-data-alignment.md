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

四个必补字段按「可公布字段清零率」验收：官方来源已公布的值必须全部补齐；官方未公布的字段允许缺省，但必须进入缺失归因清单。判定命令：

```bash
AIDOG_REGISTRY_COVERAGE_MIN=90 node scripts/check-registry.mjs
```

默认不带 env 只报数不拦截；thinking_*/predecessor 只统计不设门。门禁全绿且可公布字段缺失归因清单为空 = 循环终止。

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
- 2026-09-24 第 8 轮 litellm `7f8da6fa`（158 文件）：131 条对齐官方价目
  （litellm 平台真值源=BerriAI 官方 JSON，zai/MiniMax/deepseek 固定倍数旧价整族
  刷新；context_window 语义定为=官方 max_input_tokens）；39 条官方删键下架
  （旧世代 claude/kimi/mistral）；10 新模型。中间过程绞坏 14 文件后重建
  （教训：price 块尾无逗号时插入要处理两种形状——已在多轮重犯，考虑给
  check-registry 加预检或写公共编辑工具）。
- 2026-09-24 第 9 轮 gemini `1bb3ada7`+`219e1c2`：ctx 数量级修正 4、max_out 65535→65536
  ×7、image 系出价改 per-image token 价、robotics-er-2 现行价、gemma-4 付费价清零、
  thinking 填充、下架 10 条（3 条未来日期模型恢复保留）。NO_OFFICIAL_CHANNEL +3。
- 待确认攒问（已于 2026-09-24 ask-ui 拍板）：① aihubmix 官方在售但 registry 未收的旧代与非 token 模态全量镜像；②促销价用 `time_tiers`、未公布字段列入缺失归因；③ bailian_en 国际价维持摘要级证据；④ gemini Vertex 条目拆独立平台。
- 2026-09-24 第 16 轮 novita：108 条官方模型数据对齐（88 条既有条目修正价格/上下文/输出上限/输入上限/能力；新增 8 条有效模型，2 条上下文为 0 的候选不登记）；不删除 57 条无官方对应条目。Novita 为聚合/转售平台，新模型均 `official:false`；新增 Novita-only 模型加入 `NO_OFFICIAL_CHANNEL` 豁免，`check:registry` 4568 文件通过，`aidog_db` 351 tests 通过。
- DB 侧独立线：过期行清理（prune_model_entries）已提交 `95808e3`（门禁：aidog_db
  351 passed / aidog_core desktop 全过 / clippy -D warnings 零 warning，2026-09-24）。
- 2026-09-24 第 10 轮 shengsuanyun `be4fbeaa`+`aef56f4`：4 个 input 单位 bug（计费放大
  百万倍）、8 价格分叉、5 窗口修正、10 新模型、1 下架。model_list/index 串位后改用
  结构化 JSON 对账收尾；后续平台条目对账禁止文本锚点。
- 2026-09-24 第 11 轮 compshare `99d5263`：deepseek 峰谷平台窗口、qwen3-vl-flash
  三档价、3 新模型、10 条下线。LingDT/minimax-h3-context-ir 因官方未公布 ctx 撤下。
- 用户 2026-09-24 拍板 7 项：门禁改可公布字段清零率；aihubmix/openrouter/litellm 全量镜像
  所有在售条目（含旧代与非 token 模态，schema `unit` 支持 image/second/request/char）；
  gemini 的 Vertex 条目拆独立 vertex 平台；促销价用 `time_tiers` 起止档；继续按缺失量降序跑
  聚合站；per-image/per-second 条目登记并标 `unit`，计费走 fallback；bailian_en 国际价
  维持摘要级证据现值。
- 待执行队列：三平台全量镜像；Vertex 平台拆分；gemini flash/robotics-er-2 补 `time_tiers`；
  per-image 模型登记。
- 2026-09-24 第 12 轮 aihubmix 全量镜像 `0c0e115a`：+445 条（848 总量，含 image/second/char
  unit 模态）、12 价差按官方精确值修正、deprecated 15 条不登记（拍板主句限定「在售」）。
  93 条 context tiers 未写（门槛推断）。bump 脚本嵌套未跟踪目录 EISDIR bug 修复 `910b24d0`。
- 2026-09-24 q4 落地 `c98d4c36`：gemini-robotics-er-2-preview 补 `time_tiers`（2027-01-01 起
  2e-6/1e-5）；3.6/3.7/3.8-flash 原有档与拍板值一致未动。
- 2026-09-24 第 13 轮 openrouter 全量镜像 `78ecacdc`→merge `e99a8fe1`→修复 `a55403d4`：
  +455 条、free 模型 price.input=1e-8 占位、动态价 -1→0、59 条 openrouter 独有条目
  official:true。教训：agent 生成的快照条目缺 capabilities/canonical 未折叠，merge 后
  check 才暴露——镜像轮落地前必须先在 /tmp 组装副本跑 check。
- 2026-09-24 第 14 轮 litellm 全量镜像 `c4aa33a0f`→merge `34927fcce`：+3711 条（8777 文件
  总量）、440 条未登记全部带原因、存量 3 条 official:true 翻 false（纯镜像语义）。
- canonical 官方拼写规则（22+16 处统一 `3ec97578`+`8d4c9890d`）：以官方平台组多数为准
  ——claude 系连字符小写（claude-opus-5-5）；MiniMax/DeepSeek/Hunyuan/Qwen/LongCat 系官方
  CamelCase（MiniMax-M2.5、DeepSeek-V3.2）；gemma/nvidia/flux 系小写连字符；检查器折叠键
  含点号/大小写/下划线。
- 第 15 轮 crazyrouter 进行中；后续队列按缺失量：novita 276 / nvidia 254 / atlascloud 244 /
  openai 231 / siliconflow 182 / doubao 123。
- 2026-09-24 第 15 轮 crazyrouter `e907a6011`（65 文件，官方 /api/pricing 全量对齐，
  141 → 153 条）：新增 13 在售模型（gemini-3.8-flash、glm-5.3-flashx、gpt-6-luna/sol、
  gpt-image-2-t、gpt-image-2.5-flare/sunburst、kimi-k2.7-code(+highspeed)、
  mimo-v2.5-pro、mimo-v2.6-flash/pro、qwen3.8-flash）；下架 gpt-5.5-pro；49 条价格/
  分档修正。**平台计价口径（本轮确立）**：crazyrouter = new-api 聚合站，
  $/1M = model_ratio×$2×discount；registry 存折扣后实价（站内自洽，与 aihubmix
  存原价的惯例不同）。**billing_expr 模型以表达式系数×折扣为准**，model_ratio 相悖时
  证伪 ratio（gpt-6-astra 站点展示 $6.5 = expr 10×0.65 而非 ratio 5×0.65=3.25；
  gpt-5.6-sol 同）；tier-2（>272k）系数 = tier-1 的 input×2 / output×1.5 /
  cache_read×2；expr 无 cr 系数但 cache_ratio_configured=true 时 cache_read =
  条目价×cache_ratio（gpt-5.4-pro 30×1×0.85 两径同值）；expr 无 cc 且
  cache_creation_priced=false → 不写 cache_write（删 gpt-5.6-luna 误写的 8.125e-08）。
  其余：claude cache_write 尾数修正（4.063e-06→4.0625e-06 ×6、2.438e-06→2.4375e-06
  ×2）、claude-sonnet-5 整族实价、per-call 按折扣实价（kling 0.0425→0.0255、
  gpt-image-2 0.058→0.0377、nano-banana-pro 0.134→0.0737 等）、补 gpt-5.5 /
  gpt-5.4-pro 长上下文 context_tiers。peak 窗口（北京工作日 9-12/14-18）未动。
  未登记 6 条：hy-3d-3.0/3.1/component（schema capabilities 无 3D 枚举）、
  jev-1.13（endpoint 仅 decisions 类型，平台三端点不可路由）、kimi-k2.8-preview /
  mimo-v2.6-pro-ultraspeed（官方未公布 context_window）。NO_OFFICIAL_CHANNEL +5
  （gpt-6-luna/sol、gpt-image-2-t/2.5-flare/2.5-sunburst：openai 官方平台 registry
  滞后未收，官方一手价无来源禁以聚合折扣价代写；openai 对齐轮补条后应移除）。
  遗留：crazyrouter 详情页 thinking 逐页核对未做；gpt-image-2-t 命名与 OpenAI
  官方 id 对应关系未核。门禁：bump 62 文件 / check-registry 4572 文件通过 /
  aidog_db 351 passed 全绿。官方来源：
  https://crazyrouter.com/api/pricing（主）+
  https://r.jina.ai/https://crazyrouter.com/pricing（展示价交叉验证）；
  /v1/models 401（需令牌）。
