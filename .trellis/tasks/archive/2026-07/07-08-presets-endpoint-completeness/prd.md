# 补全 platform-presets 协议端点 + base_url + 模型

## Goal

用户发现 `platform-presets.json` 多协议端点/base_url/模型信息不全（例：阶跃星辰 stepfun 仅 anthropic，实际还支持 openai/openai_responses）。全量检索补全 60 协议。

## Scope (用户裁定)

- **范围**: 全部 60 协议
- **补全内容**: endpoints（补缺失协议端点 + 修正 base_url）+ models.default.default + model_list.default
- **依赖**: depends_on `07-08-remove-coding-plan-branch`（同文件，必须先 finish 释放 worktree）

## Requirements

1. 每协议经官网/文档核实：
   - 支持哪些协议（anthropic / openai / openai_responses / gemini）
   - 各协议 base_url（含版本前缀）
   - 默认模型 + model_list（≥3 个当前可用模型）
2. 第三方中转站（cherryin/pateway/ccsub 等纯 Claude 转发）：若官网无 openai 端点信息，保留仅 anthropic（标 `需要: 用户` 不强凑）
3. 改 `src-tauri/defaults/platform-presets.json`，保持结构（endpoints.default 数组 + client_type 字段）
4. 不破坏 CLAUDE.md 约束（base_url 含版本前缀；coding_plan 分支已由前置 task 删除）

## Acceptance Criteria

- [ ] 60 协议每条 endpoint 标注来源（官网 URL / 文档页）
- [ ] 缺失协议端点补全（如 stepfun +openai/openai_responses）
- [ ] models.default.default 非 `?`（除确认无公开信息的第三方中转）
- [ ] `python3 -c "json.load(...)"` 解析成功
- [ ] `yarn build` + `cargo check` 通过
- [ ] 无法核实的协议列入 `需要: 用户` 清单

## Technical Notes

- 真值源: `src-tauri/defaults/platform-presets.json`（顶层 `{"protocols": {...}}`）
- 已知缺口: stepfun/stepfun_en(1 ep), bailian_coding(1), qianfan(1), bailing(1), siliconflow/siliconflow_en(1), modelscope(1), sensenova(2 缺 anthropic? 待核实)
- 第三方中转(~30): 多为 Claude Code 账号池中转，官网信息少，预期部分标 `需要: 用户`
- URL 构造规则（CLAUDE.md）: base_url 含版本前缀（/v1, /api/paas/v4），provider_api_path 只 /chat/completions
- 并发: remove-coding-plan-branch finish 后释放 worktree，本 task exec 按 4 批 research 并行（原厂/聚合/中转A-L/中转M-Z）

## 用户追加（2026-07-08）

- **模型补全确认**：models.default.default + model_list.default 一并补（已覆盖 Scope）
- **价格同步**：`src-tauri/defaults/models.json` 同步更新
  - 结构: `{version, generated_at, models: {<id>: {default_platform, input_cost_per_token, output_cost_per_token, cache_read_input_token_cost, cache_write...}}}`
  - preset 新增的模型 id 必须在 models.json 有对应价格条目
  - 现有价格校准（若官网价变动）

## 执行拆分（research-heavy）

remove-coding-plan-branch 已 finish，worktree 释放。本 task start 后按 4 批 research subagent 并行（并发上限 2，分 2 轮）：

1. **原厂大模型** (15): stepfun/siliconflow/modelscope/qianfan/bailing/sensenova/bailian_coding/glm/glm_en/kimi/minimax/minimax_en/doubao/byteplus/longcat/xiaomi_mimo/deepseek
2. **知名聚合** (13): openrouter/aihubmix/dmxapi/novita/atlascloud/shengsuanyun/therouter/rightcode/packycode/cubence/aigocode/aicodemirror/nvidia/newapi
3. **第三方中转 A-L** (~15): cherryin/pateway/ccsub/apikeyfun/apinebula/sudocode/claudeapi/claudecn/runapi/relaxycode/crazyrouter/sssaicode/compshare/compshare_coding
4. **第三方中转 M-Z** (~15): micu/ctok/eflowcode/lemondata/pipellm/opencode/opencode_zen/eflowcode/lemondata 等

每批输出: `research/batch-N.md`（每协议: 支持协议/base_url/模型列表/价格 + 来源 URL）→ main 汇总 → trellis-implement 改两 JSON。

第三方中转查不到的标 `需要: 用户`，不强凑。
