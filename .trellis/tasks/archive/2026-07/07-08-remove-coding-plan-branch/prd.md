# 删 platform-presets coding_plan 分支去重

## Goal

用户反馈火山引擎（doubao）UI 显示 "anthropic" 和 "anthropic · cp" 两条，视为重复。根因：preset JSON 的 `endpoints.coding_plan` 分支使同协议在 default + cp 两个数组各出现一次，UI 渲染成两条。用户要求每协议只保留一份（default 分支）。

## Scope (用户裁定)

- **方向**: 删 cp 分支，留 default
- **范围**: 全部 8 个带 coding_plan 拆分的协议 = glm / kimi / minimax / minimax_en / bailian / qianfan / xiaomi_mimo / doubao

## Requirements

1. 从 `src-tauri/defaults/platform-presets.json` 的 8 协议删除：
   - `endpoints.coding_plan`（全部 8 协议）
   - `models.coding_plan`（7 协议，doubao 无此键）
   - `model_list.coding_plan`（7 协议，doubao 无此键）
2. 保留所有 Rust/TS 代码不动：
   - `coding_plan: true` endpoint flag 机制（用户级 `platform.extra` 仍可手工启用）
   - `defaults.ts::pickBranch` 缺 cp 分支自动回落 default（已实现 line 85）
   - `endpoint.rs` 路由：`has_coding_ep=false` 自动走普通平台路径（step 3/4）
3. CLAUDE.md 更新 coding_plan 段：preset 默认不再带 cp 分支（架构变更）

## Out of Scope

- 不删 `est_coding_plan`（用户运行期配额数据列，与 preset 无关）
- 不动 Rust `coding_plan` 字段 / quota / estimate / model_test 逻辑（向后兼容用户已配 cp 端点的平台）
- 不删前端 PlatformCard "Code" 徽标（用户级 cp 端点仍需展示）

## Acceptance Criteria

- [ ] 8 协议 preset JSON 无 `coding_plan` 键（endpoints/models/model_list 三处）
- [ ] `python3 -c "json.load(...)"` 解析成功
- [ ] `yarn build` 通过（tsc + vite，验证 defaults.ts pickBranch 回落逻辑）
- [ ] `cd src-tauri && cargo check` 通过
- [ ] CLAUDE.md coding_plan 段更新
- [ ] grep 验证 preset JSON 已无 `"coding_plan":` 键（除顶层无关字段）

## Technical Notes

- 文件: `src-tauri/defaults/platform-presets.json`（单文件多处删）
- 依赖代码（不动）: `src-tauri/src/gateway/proxy/endpoint.rs:99-119`, `src/domains/platforms/defaults.ts:81-95`, `src-tauri/src/commands/model_test.rs:63-76`
- CLAUDE.md: `### 平台默认配置 (platform-presets.json)` 段 coding_plan 子项
