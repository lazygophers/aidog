# 实施计划 — peak models 分支 review 修复

## 改动面 (3 文件 + 1 preset + 1 文档)

### 1. `src-tauri/crates/aidog_core/src/gateway/router/candidates.rs`
- `resolve_effective_models` fn 注释: 在 tier 2 peak 段加一行 — "**设计意图: peak 分支覆盖用户 platform.models 定制 (preset 级硬约束, 等同 coding_plan 端点维度优先级)**". 非行为变更.

### 2. `src-tauri/defaults/platform-presets.json`
- `protocols.glm_coding.model_list` 加 `peak` 分支: `["glm-4.7", "glm-4.6", "glm-4.5"]` (对齐 `models.peak` 槽位值集合, 去重排序).
- **禁机器覆盖**: 手编, 与 `models.peak` 同步.

### 3. `src/domains/platforms/defaults.ts`
- `DefaultsDoc.model_list` 类型: `{ default?: string[]; coding_plan?: string[] }` → 加 `peak?: string[]`.
- `getDefaultModelList(protocol, codingPlan?, isPeak?)`: 加第 3 参, 内部从 `pickBranch` 切 `pickModelsBranch`.
- 注释更新: pickBranch 仍处理 endpoints (无 peak); model_list 走 pickModelsBranch.

### 4. caller 审 (grep `getDefaultModelList`)
- `PlatformCard.tsx`: `defaultModels` useEffect 已算 `isPeak` → 传给 `getDefaultModelList` (若该处用到). 若仅用 `getDefaultModels`, 无需动.
- `formSections.tsx:452` 区: 评估是否需 isPeak (模型下拉冷启动场景).
- 所有 caller 必 `await` (CLAUDE.md 硬规).

### 5. `CLAUDE.md`
- `peak_hours` 小节: 加 `models.peak` 段 (per-protocol 可选, 命中窗口时路由层 `resolve_effective_models` 用 peak 替换 effective_models; 覆盖用户 platform.models 定制 = 设计意图; 仅 glm_coding 现带).
- "前端 N 函数": 4 → 5 (加 `getDefaultPeakHours`); 标 `getDefaultModels`/`getDefaultModelList` 第 3 参 `isPeak`.

## 验证顺序

1. 改 preset JSON → `jq empty` parse 检查.
2. 改 defaults.ts → `yarn build` (tsc 捕获 caller 签名).
3. 改 candidates.rs 注释 → `cargo clippy -p aidog_core` + `cargo test -p aidog_core`.
4. 改 CLAUDE.md → grep 验关键词.

## 风险

- `getDefaultModelList` 加参 → 向后兼容 (optional), 旧 caller 不破. 仅需主动传 isPeak 的 caller 改.
- CLAUDE.md 是 checked-in → 走工作树 diff, 用户 review, 不主动 commit (按全局规, 但本项目授权 auto commit, 见项目 CLAUDE.md).
