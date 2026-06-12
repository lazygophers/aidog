# 默认 StatusLine 3 行布局 + 配色 + coding-plan/余额 色逻辑

## Goal

把内置默认 statusline（DEFAULT_SEGMENTS，首次/重置时呈现）改为用户指定的 3 行布局 + 精确配色；新增 git-branch 段（脚本内跑 git）；增强 group-info 端点提供余额可用天数 + coding-plan 速率/重置，驱动第 3 行动态色。

## 目标布局（逐字）

**第 1 行**：`{模型,蓝} · {当前session tokens,紫}[{总花费,灰}]·{ctx%,绿}·缓存 {缓存%,绿,小数≤4位}%`
**第 2 行**：`{分支名,黄}{若有 worktree: ·{worktree}}|{pwd}`
**第 3 行**：`{coding plan/余额,百分比展示,动态色}· {版本,灰}`

## Decisions（brainstorm 已定）

- 分支名：新增 `git-branch` 段，脚本内 `git -C "$cwd" branch --show-current`（非 git/无 git 降级空）。
- 后端：**允许增强 group-info 端点** —— 补近 7 天花费速率（算余额可用天数）+ coding-plan 各 tier 利用率/预期速率/reset 时间。
- 当前 session tokens = `context_window.total_input_tokens + total_output_tokens`（当前值）。
- 改**内置默认 DEFAULT_SEGMENTS** 布局（用户仍可改）。

## 配色（语义→建议 hex，对齐主题）

| 名 | hex | 用于 |
|----|-----|------|
| 蓝 | `#4A9EFF` | 模型 |
| 紫 | `#BF5AF2` | session tokens |
| 灰 | `#8E8E93` | 总花费 / 版本 |
| 绿 | `#34C759` | ctx% / 缓存% / 第3行正常 |
| 黄 | `#FFD60A` | 分支名 / 第3行警戒 |
| 红 | `#FF453A` | 第3行危险 |

## Requirements

### 后端增强（R1，group-info 端点）
- R1.1 端点返回补充字段：
  - `balance_days_remaining`(f64|null)：余额 / 近 7 天日均花费（7d 总花费/7）；无花费/无余额 → null。
  - `coding_plan` 各 tier 补 `pace`("fast"|"normal"|"busy")+`reset_at`(unix)：pace 由「该 tier 利用率随时间的预期消耗速率」判定——平均使用快于配额时间线 → fast；接近 → normal；慢/闲 → busy。（利用 est_coding_plan 的 coef_per_token / util + 窗口时间推算；拿不到则 normal 降级。）
- R1.2 近 7 天花费：复用 proxy_log 该平台 7 天 SUM(est_cost)（db 查询，只读）。
- R1.3 不破坏端点既有字段/鉴权/只读语义。

### 新段（R2，editors.tsx）
- R2.1 `git-branch`：脚本 `b=$(git -C "${cwd}" branch --show-current 2>/dev/null); [ -z "$b" ] && exit 0; echo -n "$b"`（cwd 来自 `.workspace.current_dir`）。固定色支持。降级空。
- R2.2 增强 group 段以支持第 3 行动态色 + reset 展示：
  - **coding-plan 段**：按 `pace` 上色——fast→红、normal→黄、busy→绿。红色时**额外展示预期重置时间**（reset_at→人类可读，如 `(重置 14:30)` 或剩余时长）。
  - **余额段**：按 `balance_days_remaining` 上色——<1 天→红、<3 天→黄、否则绿（默认绿）。
  - 一个平台是 coding-plan 还是余额：端点已知（有 coding_plan tiers → coding；否则余额）。第 3 行用一个「coding-plan 或余额」段（按平台类型择一展示），或两段各自降级（无对应数据 exit 0）。
- R2.3 缓存%段：≤4 位小数百分比。缓存命中率定义：`cache_read_input_tokens / (input_tokens + cache_read_input_tokens) * 100`（用 context_window.current_usage；null 降级）。用 printf 控小数位 ≤4。

### 默认布局（R3）
- R3.1 改 `DEFAULT_SEGMENTS`（main statusline）为上述 3 行：用对应原子段 + newline 分行 + 各段固定 color（hex）+ 第1行 session-tokens 与 cost 间无分隔符（`[cost]` 紧贴，用括号包裹便捷段或 cost 段加方括号 option）+ `·`/`|` 分隔。
  - 注意：全局分隔符是统一一种；但布局里既有 `·` 又有 `|`（行 2）。需用「无全局分隔符 + 段自带前后缀/字面分隔段」实现混合分隔，或行内分隔符按需。**优先**：默认布局把全局分隔符设空，分隔靠段的 prefix/suffix option 或字面（保证 `·` 与 `|` 混排 + `[cost]` 紧贴 + worktree 条件前缀 `·`）。
- R3.2 worktree 段条件前缀 `·`：有 worktree 才显 `·{worktree}`（worktree 段本身缺失降级空，前缀随之不显）。
- R3.3 默认布局应在「加载推荐/重置」或首次无配置时生效；不破坏用户已自定义的配置。

### 通用（R4）
- R4.1 i18n 文案 t()；段固定色用 hex（数据值，非主题变量——statusline 输出 ANSI 真彩，已有 hexToRgb 机制）。
- R4.2 cargo check + tsc 0；bash -n + 实测降级。

## Acceptance Criteria

- [ ] 默认 statusline 渲染为 3 行，配色符合规格（模型蓝/tokens紫/cost灰/ctx绿/缓存绿/分支黄/版本灰）。
- [ ] 第1行 `模型 · tokens[cost]·ctx%·缓存 X%`，缓存%≤4 位小数。
- [ ] 第2行 `分支|pwd`，有 worktree 时 `分支·worktree|pwd`。
- [ ] 第3行 coding-plan/余额 动态色（coding: fast红/normal黄/busy绿 + 红时显 reset；余额: <1天红/<3天黄/否则绿）· 版本灰。
- [ ] git-branch 段跑 git 命令、非 git 降级空。
- [ ] 端点补 balance_days_remaining + coding tier pace/reset_at，只读不破坏既有。
- [ ] 旧用户自定义配置不被覆盖；仅默认/重置用新布局。
- [ ] cargo check + tsc 0；bash -n + 降级实测；i18n 7 语言。

## Out of Scope

- 不改其他段既有行为；不改全局分隔符机制本身（仅默认布局用法）。
- 不去上游真查（仍本地预估）。

## Technical Notes

- 端点：`src-tauri/src/gateway/proxy.rs`(/__aidog/group-info handler) + `db.rs`(7d SUM(est_cost) 查询 + 既有 get_group_usage_stats)；coding pace 用 `estimate.rs` EstCodingPlan(coef_per_token/util/tokens_since_real) 推算。
- 段 + DEFAULT_SEGMENTS：`src/components/settings/editors.tsx`（atomic 段已就绪：model/context-tokens/cost-usd/context-pct/context-cache/cwd/worktree-*/version/group-coding/group-balance；新增 git-branch）。
- 混合分隔：默认布局全局 sep 设空，靠段 prefix/suffix（fields option）或字面分隔。
- 复用记忆 [[shared-ui-formatters]] [[pricing-resolve-single-source]] [[group-stats-aggregation]]。
