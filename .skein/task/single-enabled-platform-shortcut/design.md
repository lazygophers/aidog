# 单启用平台组短路 — 详细设计

## 1. Rust 侧：共享 helper

新增于 `src-tauri/crates/aidog_core/src/gateway/router/mod.rs`（`candidate_state` 附近，
两个 caller 都能 `use` 到；`group_info.rs` 走 `crate::gateway::router::sole_platform`）：

```rust
/// 分组内「唯一可选平台」判定 —— 单平台分组短路的唯一真值源。
///
/// 两个分支：
/// 1. 组内平台总数 == 1 —— 物理单平台，无论 status 为何都短路（保「唯一平台 auto_disabled
///    仍必请求」的既有契约，见 test_candidates.rs）。
/// 2. 组内 `status == Enabled` 的平台恰好 1 个 —— 用户只启用了一个，等效单平台分组。
///
/// 与 status 正交的临时闸门（expires_at 过期 / disable_during_peak）**不参与本判定**，
/// 由各自路径处理（短路后 handle_single_platform 仍会做高峰禁用硬停）。
pub(crate) fn sole_platform(gps: &[GroupPlatformDetail]) -> Option<&GroupPlatformDetail> {
    if gps.len() == 1 {
        return Some(&gps[0]);
    }
    let mut it = gps.iter().filter(|gp| gp.platform.status == PlatformStatus::Enabled);
    match (it.next(), it.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}
```

`(it.next(), it.next())` 双取模式避免 collect 分配，且天然表达「恰好一个」。

## 2. 调用点改造

### 2.1 `gateway/router/candidates.rs:185-191`

```rust
// ── 阶段 0: 单启用平台分组短路 ──
if let Some(only) = sole_platform(&group_platforms) {
    return handle_single_platform(
        db, group, only, source_model, ctx,
        &mapped_target_model, mapping, now_ms, &extra_cache, &cli_cache
    ).await;
}
```

`handle_single_platform` 函数体**零改动** —— 它已按「唯一平台」语义处理手动 Disabled 硬停 /
cli-proxy provider 缺失硬停 / 高峰禁用硬停 / status bypass。新口径下传入的平台必为
Enabled（分支 2）或组内唯一（分支 1），两种情况原逻辑都成立。

⚠️ 注意：分支 2 下 `only.platform.status == Enabled`，故 `:277` 的手动 Disabled 硬停不会命中，
`:314` 的 "bypassing status filter" 日志语义仍准确（bypass 的是熔断/其他维度）。

### 2.2 `gateway/proxy/group_info.rs:110-121`

```rust
let platforms = match super::db::get_group_platforms(&state.db, group.id).await { ... };
let Some(gp) = crate::gateway::router::sole_platform(&platforms) else {
    ok_empty!();
};
let platform = &gp.platform;
```

`ok_empty!()` 是既有宏（早退返 `applicable:false`），沿用。

## 3. 前端：GroupIcon

`src/domains/groups/GroupIcon.tsx:8` 同口径 TS 版（**与 Rust 对称，改一处必改另一处**）：

```ts
const enabled = gps.filter((gp) => gp.platform.status === "enabled");
const single = gps.length === 1 ? gps[0].platform : enabled.length === 1 ? enabled[0].platform : null;
```

`status` 字段已存在于生成类型（`src/services/api/types/generated/Platform.ts:37`），
序列化值为 `"enabled" | "disabled" | "auto_disabled"`（`protocol.rs:301-309` serde rename）。

## 4. 前端：statusline 文案

`src/components/settings/statusline-segments.ts` 5 处 + `src/locales/*.json` 8 语言 × 6 条
`statusline.seg.group-*` 的 desc，把「仅单平台分组」措辞改为「仅单启用平台分组」。
**纯文案，无逻辑**。locale 走顶层扁平 dotted key，注入保序禁全排（memory `locale-flat-key-convention`）。

## 5. 关键取舍

| 取舍 | 选择 | 理由 |
|---|---|---|
| helper 放哪 | `router/mod.rs` `pub(crate)` | 两个 caller（router / proxy）都在 gateway 下；放 models 会让 models 依赖 status 语义 |
| 是否保留 `len()==1` 兜底 | 保留 | 用户 grill W1 拍板；不保留则 2 条现有契约测试反转 |
| auto_disabled 试探通道 | 放弃（短路优先） | 用户 grill W2 拍板；实现最简，行为最可预测 |
| `handle_single_platform` 是否改签名 | 不改 | 已收 `&GroupPlatformDetail`，helper 返回值直接喂入，零改动 |
| 前端是否复用后端判定 | 不复用，TS 独立实现 | GroupIcon 是纯展示，为它加一个 IPC 往返不值；靠 design 文档 + 注释锁对称性 |

## 6. 风险

- **前后端口径漂移**：TS 与 Rust 各一份判定。缓解 = 两处均加注释互指 + design 表格锁定。
- **文案与行为脱节**：statusline 文案说「仅单启用平台分组」，其行为由 group-info 的 `applicable`
  决定 —— 二者同批改，check 阶段需交叉核对措辞与 `sole_platform` 语义一致。
