# 修复 coding plan 配额耗尽(util≥100)配色应为红

## 背景

UI 显示 "0%week·1d 23h"（utilization=100%，剩余配额=0），但颜色为绿。

根因：`coding_tier_level` / `codingTierLevel` 用 pace 算法算「剩余可用时间%」:
```
pace = util_ratio / elapsed_ratio
remain_pct = 100 / pace
```
当 util=100%（util_ratio=1.0），weekly 周期剩余 1d23h（elapsed≈0.72）→ pace≈1.39 → remain_pct≈72 → 绿。

pace 算法衡量「按当前燃烧速度能否撑到周期末」，但**配额已耗尽**时该语义失效 —— 当前已无可用配额，撑不撑得到无意义。算法此时按「时间维度燃烧不快」给绿，与现实矛盾。

## 修复目标

utilization ≥ 100（剩余配额=0）→ 强制 `Red`（danger），绕过 pace 算法。前后端单一事实源同步。

## 改动范围

| 文件 | 改动 |
| --- | --- |
| `src-tauri/src/gateway/usage_color.rs` | `coding_tier_level` 入口加 `utilization >= 100.0 → Red` 守卫；新增 test |
| `src/components/shared/usageColor.ts` | `codingTierLevel` 入口加 `utilization >= 100 → "danger"` 守卫 |

不动 `Platforms.tsx`、`proxy.rs`、`estimate.rs`（调用站点不变，守卫在单一函数内全覆盖 4 处调用）。

## 验证

- `cd src-tauri && cargo test` —— 含新增 `coding_tier_depleted_is_red` 通过
- `yarn build` —— tsc 通过
- 边界：
  - util=100 → Red
  - util=99.9 → 走原 pace 算法（非本修复目标，保持现状）
  - util=150（异常上溢）→ Red
  - 缺 remain/cycle → 仍 Neutral（守卫在 Neutral 判定之后）

## 非目标

- 不调阈值（CODING_REMAIN_PCT_DANGER/WARN 不动）
- 不改 statusline 文案
