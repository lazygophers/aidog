# Implement — platform-presets 全面检修

> subtask DAG 见 `design.md` §6。共享 task worktree（1 task 1 worktree，subtask 不绑定）。并发上限 2，JSON 改动串行。

## ST1 — PeakWindow.models schema 跨层

| 要素 | 内容 |
|---|---|
| 目标 | PeakWindow 增 `models: Option<Vec<String>>`（absent=全平台），hit 逻辑加 model 过滤 |
| 产出 | `peak_hours.rs`（struct + serde + resolve_multiplier + is_in_peak_window + model_match）+ `defaults.ts::PeakWindow` 类型 + `peakHours.ts::hit` 对称 + Rust unit test（model scope hit 边界） |
| 验证 | `cargo test -p gateway peak_hours` 全过 + grep 跨层字段名一致 + clippy 0 新增 |
| 资源 | `.trellis/spec/guides/cross-layer-rules.md`（强制读） |
| 依赖 | 无（先行） |

model_match 语义：exact-first，`glm-5.2*` 覆盖 `glm-5.2` / `glm-5.2-turbo`。resolve_multiplier 签名增 `request_model: &str`。

## ST2 — 消费链适配 model 参数

| 要素 | 内容 |
|---|---|
| 目标 | resolve_multiplier / is_in_peak_window 调用点传 request_model |
| 产出 | `estimate.rs`（est_cost 传 proxy_log.model）、`stats_today.rs:230`（传 model）、`router/mod.rs:81`（disable_during_peak 传 request model）、`proxy/handler.rs`（log 传 model） |
| 验证 | cargo build + clippy + grep 全调用点无漏（resolve_multiplier / is_in_peak_window caller 全更新） |
| 资源 | ST1 产物 |
| 依赖 | ST1 |

## ST3 — glm-coding 独立协议

| 要素 | 内容 |
|---|---|
| 目标 | platform-presets.json 增 glm-coding 条目 + glm 删 peak_hours/clean slot + 前端 constants/PROTOCOLS/matchPlatform + CLAUDE.md 改 |
| 产出 | JSON 两处改 + `constants.ts` 增 glm-coding + `PROTOCOLS` / 协议枚举同步 + CLAUDE.md「coding_plan 分支已删」段改写 |
| 验证 | JSON parse OK + 前端 getDefaultPeakHours("glm-coding") 返 model-scoped 窗口 + yarn build 0 错 + CLAUDE.md grep 无矛盾 |
| 资源 | design §2，glm coding plan 文档（ST6 research 辅助 base_url） |
| 依赖 | ST1（schema 定型后 JSON 用 models 字段）；与 ST5/ST7 串行（同文件） |

注：base_url 若 ST6 未完先用预填 `/api/paas/v4`，ST7 校准。

## ST4 — 前端 UI model scope

| 要素 | 内容 |
|---|---|
| 目标 | PeakHoursSection 增 model 多选编辑 + PlatformCard 徽标显示受影响模型 |
| 产出 | `formSections.tsx::PeakHoursSection`（窗口编辑增 model chip 多选 from model_list / 自由输入）+ `PlatformCard` 徽标「高峰」hover/tooltip 列受影响模型 + i18n 8 locale |
| 验证 | yarn build + check-i18n 0 缺失 + 手动验证 UI（model scope 可编辑可显示） |
| 资源 | ST1 类型 |
| 依赖 | ST1 |

## ST5 — 清非标准 slot（19 协议）

| 要素 | 内容 |
|---|---|
| 目标 | models JSON 删 fast/thinking/coder 非白名单 slot，模型去留 per 协议裁定 |
| 产出 | platform-presets.json 19 协议 models 清理（design §3 策略） |
| 验证 | python grep `'"fast":|"thinking":|"coder":'` 在 models 段 = 0 + JSON parse + yarn build |
| 资源 | design §3，per 协议 model 去留表（subtask 内列） |
| 依赖 | ST3（同文件串行） |

## ST6 — 全协议 research 分批（只读）

| 要素 | 内容 |
|---|---|
| 目标 | 60 协议 endpoints/models/model_list 核对官方文档，列 diff + 补齐建议 |
| 产出 | `.trellis/tasks/07-09-platform-presets-overhaul/research/<protocol>.md`（头部 12 重点 + 长尾最佳effort） |
| 验证 | 头部 12 每协议有 source URL + diff 表 |
| 资源 | 官方文档（WebSearch / WebFetch） |
| 依赖 | 无（早启动，与 ST1-ST5 并行只读） |

## ST7 — 数据补齐落盘

| 要素 | 内容 |
|---|---|
| 目标 | 依 ST6 research 改 platform-presets.json（model_list 补 / base_url 校准 / models slot 值） |
| 产出 | JSON 60 协议更新 + last_updated 更新 |
| 验证 | JSON parse + 每协议与 research md 对账 + yarn build |
| 资源 | ST6 research |
| 依赖 | ST5（同文件串行），ST6 |

## ST8 — check 闭环

| 要素 | 内容 |
|---|---|
| 目标 | 全量质量门 + 跨层对称 grep + CLAUDE.md/spec 同步 |
| 产出 | 问题修复 + 报告 |
| 验证 | cargo test（含新 model scope 用例）+ cargo clippy 0 新增 + yarn build 0 错 + check-i18n 0 缺失 + 跨层 grep（PeakWindow.models / resolve_multiplier 签名对称） |
| 资源 | ST1-ST7 全完 |
| 依赖 | ST2 / ST4 / ST7 |

## 调度

并发上限 2。JSON 串行（ST3→ST5→ST7）。

```
T0: ST1（schema 先行）
T1: ST2 + ST4（schema 完，并行，非 JSON）
T0+: ST6（research，全程只读并行，独立 subagent）
T2: ST3（JSON 串行第 1）
T3: ST5（JSON 串行第 2）
T4: ST7（JSON 串行第 3，依 ST6）
T5: ST8（check）
```

ST6 早启动（与 ST1/ST2/ST4 并行），不阻塞 JSON 串行链（ST3 可用预填，ST7 才需 ST6）。
