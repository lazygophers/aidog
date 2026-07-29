# 模型单价时间维度化 — 详细设计

## 1. JSON 契约（用户 2026-07-29 逐字拍板）

`price_data` 加可选 `time_tiers` 数组。**每个条目是一张完整价表** ——
自带 base 三价 + 可选内嵌 `context_tiers`。

🔒 **挂在 `pricing.glm_coding` 节点下，不放 top-level**（用户拍板：涨价限定
GLM Coding Plan，与 W2 原语义逐字等价；走普通 `glm` 协议的平台价格不变）。
`resolve_price` 档 1 查 `pricing[platform_type]`，`platform_type` 实参是
`Protocol::wire_str()`（`proxy/log.rs:147,331`），`GlmCoding` 的 wire 名 = `glm_coding`
（CLAUDE.md serde 约定）→ 该节点会被档 1 命中。

glm-5.2 / glm-5-turbo 现在 `pricing` 只有 `glm` / `openrouter` 两 key，
glm_coding 平台实际落 top_level 档（档 2）。本 task **新建 `pricing.glm_coding` 节点**，
其 base 三价 **必须逐位等于 top-level 现价** —— 否则未到涨价日就变价。

```json
"pricing": {
  "glm": { ...不动... },
  "openrouter": { ...不动... },
  "glm_coding": {
    "input_cost_per_token": 6.944444444444444e-07,
    "output_cost_per_token": 3.055555555555555e-06,
    "cache_read_input_token_cost": 1.6666666666666665e-07,
    "time_tiers": [{
      "start_at": 1790784000,
      "input_cost_per_token": 1.3888888888888888e-06,
      "output_cost_per_token": 6.111111111111111e-06,
      "cache_read_input_token_cost": 3.333333333333333e-07,
      "context_tiers": [{
        "min_tokens": 32768,
        "input_cost_per_token": 1.9444444444444444e-06,
        "output_cost_per_token": 7.222222222222222e-06,
        "cache_read_input_token_cost": 5.0e-07
      }]
    }]
  }
}
```

（上例为 glm-5-turbo；glm-5.2 同形状但 `context_tiers` 为空 —— 其顶层
`context_tiers` 就是 `[]`，time 条目里也不写。）

- `start_at`：Unix **秒**（与 `peak_hours.start_at` 同单位，`peak_hours.rs:156-166`）
- 多档时选 `start_at <= now` 中**最大**的一个（与 `apply_context_tier` 的
  `max_by_key` 选档 idiom 逐字对称）
- 条目内三价字段各自可选：缺省则继承 base（同 context tier 的 null 继承语义）
- **内嵌 `context_tiers` 而非复用顶层** —— 涨价日后长文档也涨，两张表分别列全
- **仍支持 top-level `time_tiers`**（`apply_tiers` 先查当前档节点、缺失回落 pd）——
  零额外代码就留下「模型级时段价」的口子，本 task 不用

## 2. Rust：把 `apply_context_tier` 包成 `apply_tiers`

现状：`resolve_price` 的 4 档回退链里前 3 档各调一次
`apply_context_tier(base, &pd, input_tokens)`（`model_price.rs:199 / :217 / :236`），
它从 `pd["context_tiers"]` 读档。

改法 —— **不改 `apply_context_tier` 一行**，外面套一层选表：

`apply_tiers` 多收一个 `scope` = **当前命中档的节点**（档 1/3 是 `pricing[x]`，
档 2 是 `pd` 自身），time_tiers 先查 scope、缺失回落 pd：

```rust
/// 时间阶梯选档：取 `time_tiers` 中 `start_at * 1000 <= now_ms` 的最大档。
/// time_tiers 先查当前价档节点 `scope`（= pricing[platform_type]，平台级时段价），
/// 缺失回落 `pd` 顶层（模型级时段价）。命中后该条目整体作为价表
/// （base 三价覆盖 + 其内嵌 context_tiers 替代顶层），再跑 context 分档 ——
/// 顺序 time→context，因为涨价后的长文档价只能表达在 time 条目内部。
/// `now_ms <= 0` = 无时间上下文，跳过（同 est_cost_from:98 约定）。
pub(crate) fn apply_tiers(
    mut base: ResolvedPrice,
    scope: &serde_json::Value,
    pd: &serde_json::Value,
    input_tokens: i64,
    now_ms: i64,
) -> ResolvedPrice {
    let hit = (now_ms > 0)
        .then(|| {
            scope.get("time_tiers")
                .or_else(|| pd.get("time_tiers"))
                .and_then(|v| v.as_array())
        })
        .flatten()
        .and_then(|tiers| {
            tiers.iter()
                .filter_map(|t| {
                    let at = t.get("start_at").and_then(|v| v.as_i64())?;
                    (at.saturating_mul(1000) <= now_ms).then_some((at, t))
                })
                .max_by_key(|(at, _)| *at)
        });
    let ctx_src = match hit {
        Some((_, tier)) => {
            overlay_prices(&mut base, tier);   // 非 null 字段覆盖，null 继承
            base.source.push_str("+time");
            tier                                // ← context 分档改从 time 条目读
        }
        None => pd,                             // 未命中：现状不变
    };
    apply_context_tier(base, ctx_src, input_tokens)
}
```

`overlay_prices` = 把 `apply_context_tier:280-288` 那三段 `if let Some(v) = ...`
抽成 helper，两处共用（`apply_context_tier` 内部改调它，行为不变）。

调用点改写（`model_price.rs`）：

| 档 | 行 | 改后 |
|---|---|---|
| 1 `pricing[platform_type]` | `:199` | `apply_tiers(x, pricing_node, &pd, input_tokens, now_ms)` |
| 2 top_level | `:217` | `apply_tiers(x, &pd, &pd, input_tokens, now_ms)` |
| 3 `default_platform` | `:236` | `apply_tiers(x, pricing_node, &pd, input_tokens, now_ms)` |
| 4 fallback | `:250-255` | 不涉及（连 `pd` 都没有） |

未命中 time 档时 `ctx_src == pd` —— 与改动前逐字等价，既有 context_tiers 行为零回归。

## 3. Rust：`resolve_price` 签名 + 9 处调用点

```rust
pub async fn resolve_price(
    db: &Db, model_name: &str, platform_type: &str,
    fallback_input: f64, fallback_output: f64, input_tokens: i64,
    now_ms: i64,                          // ← 新增末位参数
) -> Result<ResolvedPrice, String>
```

| # | 调用点 | 传值 | 理由 |
|---|---|---|---|
| 1 | `gateway/billing.rs:40` | `created_at_ms`（同函数第 8 参，`:75` 已传给 `est_cost_from`） | 审计重放按日志自身时刻定价 |
| 2 | `gateway/estimate/db_ops.rs:196` | `gateway::db::now()`（`:239` 已在用） | 无 created_at 可取（`spawn_estimate` 未透传，`proxy/log.rs:334-346`） |
| 3 | `platform_cmd/price.rs:60`（`model_price_resolve`） | `gateway::db::now()` | 前端预览当前价 |
| 4-5 | `platform_cmd/test_price.rs:25 / :27` | `0` | 验回退链不验时段 |
| 6-9 | `gateway/db/test_model_price.rs:163/:170/:179/:183` | `0` | 同上 |

`now_ms <= 0` 语义 = **跳过 time_tiers**，让既有测试逐字不变地继续断言基准价。

## 4. Rust：models.json bundled 兜底

同一个 `pd` 取值处（`model_price.rs:187-191`），改两级：

```rust
// price_sync.rs（挨着 :16 的既有 const）—— 照抄 presets_cache.rs:12-23 idiom
const BUNDLED_MODELS: &str = include_str!("../../../defaults/models.json");
static BUNDLED: OnceLock<serde_json::Value> = OnceLock::new();

/// bundled models.json 里该模型的 price_data 节点。DB 未同步时的只读兜底。
pub(crate) fn bundled_model_entry(name: &str) -> Option<&'static serde_json::Value> {
    BUNDLED
        .get_or_init(|| serde_json::from_str(BUNDLED_MODELS).unwrap_or_default())
        .get("models")?
        .get(name)
}
```

```rust
// model_price.rs:188-191
let pd: serde_json::Value = match &mp {
    Some(m) => serde_json::from_str(&m.price_data).unwrap_or_default(),
    None => crate::gateway::price_sync::bundled_model_entry(model_name)
        .cloned()
        .unwrap_or(serde_json::Value::Null),
};
```

**四档回退链一行不改** —— bundled entry 的结构与 DB 里存的 `price_data` 同构
（都源自同一份 models.json），`pricing` / `context_tiers` / `time_tiers` 全部天然继承。

- **DB 恒优先**：`mp` 有值就用 DB，不比 `generated_at`（范围外）
- 体积：`.rodata` +368.5KB（`platform-presets.json` 105KB + `client-types.json` 23KB
  已同法 bundled，`defaults.rs:8-9` 约定不走 Tauri resources）。`OnceLock` 懒解析，不占 RAM
- **注意 `include_str!` 相对路径从当前 .rs 起算**（memory
  `include-bytes-path-from-proxy-to-crate-root`）：`gateway/price_sync.rs` →
  crate 根是 `../../`，`defaults/` 在 `src-tauri/` 下 → `../../../defaults/models.json`。
  实现时以编译通过为准

## 5. Rust：estimate 链补 peak 倍率

`estimate/db_ops.rs` 现状：`:196` 拿 `resolved_price`，`:214`（余额扣减）与
`:233`（手动预算）各用一次，**都没乘 peak 倍率** —— 与 `calc_est_cost`
（`proxy/log.rs:149` → `billing.rs:106`）口径不一致，是既存 bug。

```rust
// :199 附近，resolved_price 之后
let peak_mult = crate::gateway::peak_hours::resolve_multiplier(
    &crate::gateway::peak_hours::peak_hours_for(extra, platform_type),
    now(),
    model,
);
```

`:214` 与 `:233` 各乘一次 `peak_mult`。+4 行，零额外 DB 查询
（`extra` `:187` / `platform_type` `:183` / `model` `:186` 全是现成入参，
preset 走 `OnceLock`，`presets_cache.rs:16`）。

🔒 **禁改调 `calc_est_cost`**：`:196` 刻意传 `fallback_input=0.0, fallback_output=0.0`
（未知模型不扣余额），`calc_est_cost` 用 `settings.fallback_*`（默认 3.0 $/M）
会开始静默偷扣；且多两次 DB 查询、参数类型 `i32`/`i64` 不匹配。

## 6. Rust：`maybe_auto_sync` 接回生产

现状：`price_sync.rs:158` 标 `#[allow(dead_code)]`，生产调用点 **0**
（只有 `:206` / `:214` 两个自测）。`model_price` 表的生产写入仅两处 ——
手动「立即同步」按钮（`platform_cmd/price.rs:65-69`）与导入导出
（`import_export/apply/mod.rs:512`）。**全新安装 + 没点过按钮 = 表空**，
UI 上「自动同步价格」开关是假的。

接回点：`startup.rs` 的既有后台 spawn 区，`tokio::spawn` 一次 `maybe_auto_sync`
（函数内部已自带开关判定与间隔判定，外面不加逻辑）。删 `#[allow(dead_code)]`。
失败仅 `tracing::warn`，禁阻塞启动（冷启动是另一条 perf 任务的红线）。

## 7. 数据 migration：清存量 `extra.peak_hours` 里的 W2 副本

用户点过平台表单的「导入默认配置」（`formSections.tsx:596-600`）会把 preset 的
`peak_hours` 复制进 `platform.extra`，删 bundled 清不掉；且导出会带上
（`import_export/test_collect.rs:328-353` 证明业务键保留）→ 随备份传播。

判定条件（**三条全中才删**，宁漏勿误删用户自建窗口）：

```
start_at == 1790784000 && multiplier == 2.0 && start_hour == 0 && end_hour == 24
```

`start_at` 这个魔数是 W2 独有指纹，用户手配撞上的概率可忽略。
实现：一次性 migration，遍历 `platform.extra` JSON，命中则从数组移除；
数组清空则整个删掉 `peak_hours` 键。migration 号沿用既有 runner
（按表归属分流，memory `migration-maintenance-by-table-owner`）。

## 8. 前端

**零功能改动**（`time_tiers` 不做可视化，见 PRD 范围外）。只加护栏测试：

`src/utils/peakHours.test.ts` 新增 `isCurrentlyPeak` + `start_at` 两例 ——
窗口带未来 `start_at` → 不命中；`nowMs` 越过 `start_at` → 命中。
锁住「`start_at` 定时炸弹」这一类问题下次能被测试逮住。
`isCurrentlyPeak` 现无任何用例（`peakHours.test.ts` 只测 `shiftClock` / `normalizeWindow`）。

## 9. 关键取舍

| 取舍 | 选择 | 理由 |
|---|---|---|
| 涨价放哪 | `price_data.time_tiers` | 用户拍板；`peak_hours` 表达「周期性倍率」，永久涨价不是周期 |
| 表结构 | 不改，塞 JSON blob | `price_data` 已是 blob（`schema_early.rs:35-44`），零 migration |
| time × context 顺序 | time 选表 → context 分档 | 用户拍板；反序则扁平 time 条目表达不了二维，长文被抹回低价 |
| time 条目形状 | 完整价表 + 内嵌 `context_tiers` | 用户拍板（选项 #5） |
| `apply_context_tier` 是否改 | 不改，外面包 `apply_tiers` | 三处调用点各改一个函数名，diff 最短 |
| 无时间上下文 | `now_ms <= 0` 跳过 | 复用 `est_cost_from:98` 既有约定；6 处测试传 0 后逐字不变 |
| bundled 兜底做法 | 读侧两级取值 | 启动 seed 要写迁移+幂等+失败回滚，读侧 6 行搞定同一场景 |
| bundled vs DB 版本 | DB 恒优先，不比 `generated_at` | 用户点过同步 = 更新，bundled 只兜「从没同步过」 |
| bundled 兜底粒度 | **行级**（DB 无该模型才读 bundled） | 用户拍板；字段级会反复覆盖用户手工删字段的意图。窗口期（改动未合 master）内 DB 有旧行的用户涨价不生效，但 W2 到 2026-10-01 才生效，合并必早于此，无实际错账 |
| 涨价作用域 | `pricing.glm_coding` 节点，非 top-level | 用户拍板；W2 原本只挂 glm_coding，放 top-level 会让普通 glm 平台一起涨，语义扩大 |
| time_tiers 查找顺序 | scope（平台档）→ pd（模型级） | 一行 `.or_else`，同时支持两种作用域，不预建配置 |
| estimate 补倍率做法 | 就地乘 `peak_mult` | 改调 `calc_est_cost` 会引入 fallback 偷扣 + 两次 DB 查询 |
| devin ACU | 只加注释 | 用户拍板；ACU 是厂商实际计量，叠倍率是重复计价 |
| migration 判定 | `start_at` 魔数三条件与 | 宁漏勿误删用户自建窗口 |

## 10. 风险

- **前后端 preset 生效时机不同步**：Rust 读 bundled（重编即生效），前端读 app-data
  merged 且 deep merge 只补缺失 protocol key（`defaults.rs:36-41`）→ glm_coding 已存在
  则**旧 peak_hours 数组整体保留**。窗口期内前端徽标可能仍显示 W2、后端已不计。
  缓解 = 该窗口在 `defaults_sync` 拉到新版后自愈；且 W2 到 2026-10-01 才生效，
  本 task 在生效前落地即无实际错账。**不为此加同步逻辑。**
- **`resolve_price` 签名波及面**：9 处调用点，改 `aidog_core` 公开签名必须
  `cargo test --workspace`（memory `cargo-workspace-gate-not-single-crate`），
  `-p aidog_core` 会漏 `*_cmd` 侧调用点。
- **`models.json` 是 368.5KB 单行密集 JSON**：手改两个模型条目容易破 JSON。
  缓解 = 改后必跑 `python3 -m json.tool` 或 `jq . > /dev/null` 验证语法 +
  bundled 解析测试会在 `unwrap_or_default()` 处静默吞错 → 测试必须断言
  `bundled_model_entry("glm-5.2").is_some()` 而非只断言不 panic。
- **`maybe_auto_sync` 接回后首启会发网络请求**：可能拖慢冷启动。
  缓解 = `tokio::spawn` 后台跑 + 函数内既有间隔判定，不进启动关键路径。
