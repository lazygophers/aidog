# Implement — Coding plan tier statusline 显示修复

## 执行编排
单交付，触点集中在 `engine.py` + `coding_plan.rs` + golden fixtures，存在跨层（TS 生成 ↔ Python golden）与顺序依赖（先确认 TS/Python 同步关系再改），**单 subagent 串行执行**（不拆并行，避免 engine.py/golden 冲突）。worktree 隔离。

## 触点清单（精确到行，须复现确认）
1. `scripts/statusline-golden/engine.py:490-518` `seg_group_coding`
   - **bug1**：`worst=min(tiers)` 整行单色 → 改为 per-tier 着色后拼接（每 tier 按自身 remain% 取色），保留 dynamicColor 语义。
   - **bug2/bug5**：`:510-512` reset 格式 → d/h/m 三段进位 `{d}d{h}h{m}m`，去中文"重置 "，倒计时段灰色（非整段红）。
   - **bug4**：`:472-473` else 分支 `mcp_monthly` → 映射 `mcp（30d）`。
2. `src-tauri/src/gateway/quota/coding_plan.rs:144-207` `parse_zhipu_coding_plan` GLM `TIME_LIMIT`
   - **bug3**：核实 `usage`/`currentValue`/`remaining`/`percentage` 字段真实语义，修正 `utilization`（已用%）与 `resets_at`。用户实测 mcp 剩 100%（已用 0%）。无真实响应时标 `需要:`。
3. 同步检查 `src/components/settings/statusline-gen.ts` / `statusline-runtime.ts` 是否含 coding 段 Python 生成逻辑，若有须与 engine.py 字节一致。
4. golden fixtures `scripts/statusline-golden/` 更新 + 字节回归。
5. （仅核实）`src-tauri/src/gateway/proxy/group_info.rs:140-157` tier 下发结构（name/utilization/reset_at），bug4 标签也可在此映射——但优先 engine.py 改，避免影响 PlatformCard。

## 验收命令
- `cd src-tauri && cargo test`（coding_plan/usage_color 相关）
- `cd src-tauri && cargo clippy`（无 warning）
- statusline golden 回归（`scripts/statusline-golden/` 跑测）
- `yarn build`（tsc + vite，前端类型）

## 失败处理
- bug3 缺真实 GLM 响应 → 返回标 `需要: GLM coding plan /api/monitor/usage/quota/limit 真实响应（mcp TIME_LIMIT 项字段）`，main 转达用户。
- golden 字节不匹配 → 检查 TS 生成 vs engine.py 是否同步，二者须一致。
