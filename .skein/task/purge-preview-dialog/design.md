# 清理失效平台弹窗展示待清理清单与原因 — 详细设计

## 现状

清理入口两处，确认弹窗都只有一句泛化文案：

- 全局：`src/pages/platforms/PlatformListView.tsx:248-265`，文案 `platform.purgeDisabledConfirm`
- 分组：`src/pages/Groups/GroupListItem.tsx:541-557`，文案 `group.purgeDisabledConfirm`

后端筛选条件在 `gateway/db/platform_lifecycle.rs::purge_auto_disabled_platforms`，两类：

1. `status='auto_disabled'` 且 `last_error LIKE 'HTTP 401%' OR 'HTTP 403%'`（key 失效，
   402/429 配额类可自愈的 auto_disabled 不删）
2. `expires_at > 0 AND expires_at < now`（已过期）

分组级还多一层处置分流：本组活跃成员数 == 1 → 永久删除；> 1（共享）→ 仅删本组 `group_platform` 关联。

## 核心取舍：筛选条件必须单一真值源

preview 与 purge 若各写一份 SQL，条件漂移时弹窗会骗人（列 3 个实际删 5 个），
而这是**不可撤销**操作，骗人的代价是用户数据丢失。所以先把现有筛选查询提取为一个
共用函数（返回带 reason 与 action 的候选行），purge 与 preview 都调它，purge 拿 id 去删、
preview 直接返给前端。这是本 task 唯一的结构性改动，其余都是薄封装与 UI。

## 后端

新增只读 command `platform_purge_disabled_preview(group_id: Option<u64>)`，返回：

```rust
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeCandidate {
    pub id: u64,
    pub name: String,
    pub reason: &'static str,  // "auth_failed" | "expired"
    pub action: &'static str,  // "delete" | "unassign"
}
```

`reason` / `action` 是**稳定枚举码，不是文案** —— 后端拼中文会绕过 8 语言 i18n。
全局模式 `action` 恒为 `delete`；分组模式按活跃成员数计数（必须 `deleted_at=0` 过滤，
否则把软删关联算进来会误判独占）分流 `delete` / `unassign`。

一个平台同时满足两类条件时 `reason` 取 `auth_failed` 优先（更能说明问题根因）。

命令注册进 `src-tauri/src/startup.rs` 的 `generate_handler!`，TS 侧在
`src/services/api/platforms.ts` 加 `purgeDisabledPreview` 封装。

## 前端

两处弹窗打开时拉一次 preview（`useEffect` on open，非常驻轮询），渲染列表：

- 每行：平台名 + 原因徽标（i18n 映射 `reason`）
- 分组入口额外按 `action` 分两段（「将永久删除」/「将移出本分组」），全局入口单段
- 空清单：空态文案 + 确认键 `disabled`
- 加载中：骨架或 loading 文案，确认键 `disabled`

弹窗已是 shadcn `AlertDialog`（内部 portal 到 body），符合 CLAUDE.md 硬规，不改结构只填内容。
列表长时给 `max-height + overflow:auto`，避免弹窗撑出屏幕。

## i18n

新增 key（8 个 locale 全填）：清单标题、`reason.auth_failed`、`reason.expired`、
`action.delete` / `action.unassign` 分段标题、空态文案。`scripts/check-i18n.mjs` 必须绿。

## 测试接缝 (seam)

取最高接缝、复用现有的：`src-tauri/crates/aidog_core/src/gateway/db/test_platform_lifecycle.rs`
已有一键清理的三场景用例（全局全删 / 分组独占删 / 分组共享仅移除关联，`:133`）。
在同文件加**一个** seam 测试：同一 DB 状态下，`preview` 返回的 id 集合 == `purge` 实际
处理的 id 集合（`deletedIds ∪ unassignedIds`），且 `action` 与实际落到哪个集合一致。
这一条同时证明「条件同源」与「分组处置分流正确」，是 PRD 里最关键那条验收的直接证据。

前端不新建测试接缝：UI 渲染由 `yarn build` + `check-i18n.mjs` 覆盖，
列表渲染是纯数据映射，无独立逻辑值得单测。
