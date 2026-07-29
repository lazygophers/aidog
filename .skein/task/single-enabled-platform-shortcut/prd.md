# 单启用平台组短路 — PRD (主入口)

## 目标

用户原话：「之前的规则是，如果一个分组下只有一个平台就 xxxx，现在都修改为一个分组下只有一个**启用**的平台就 xxxx」。

把全项目「单平台分组」的判定口径从「组内平台**总数** == 1」改为「组内**启用**平台数 == 1」，
使「组里挂了 3 个平台但只启用了 1 个」的分组也能享受单平台分组的待遇
（路由 bypass 状态过滤必请求 / group-info 返本地预估值 / 前端展示该平台 logo / statusline 文案口径一致）。

成功长相：组内只启用一个平台时，行为与只挂一个平台完全一致；现有单平台组行为零回归。

## 边界

### 口径定义（用户 2026-07-29 拍板，契约锁定）

「启用」= `platform.status == PlatformStatus::Enabled`。
`auto_disabled`（401/403 退避中）**不算启用**。`expires_at` 过期 / `disable_during_peak` 命中窗口
是与 status 正交的临时闸门，**不参与本判定**（沿用各自现有处理）。

### 判定公式（唯一真值源，四处改动全部对齐）

```
sole_platform(gps) =
    if gps.len() == 1        → Some(gps[0])          // 物理单平台兜底（grill W1，保现有契约）
    else if enabled.len()==1 → Some(唯一 enabled)     // 新规则
    else                     → None
enabled = gps.filter(|gp| gp.platform.status == Enabled)
```

- [ ] **grill W1 裁定**：保留「组内平台总数 == 1」兜底分支。语义 = 「组里没有第二个可选平台」。
  这样 `test_candidates.rs:92`「唯一平台 auto_disabled 时仍必请求」契约不变，零回归。
- [ ] **grill W2 裁定**：`enabled_count == 1` 且组内另有 auto_disabled 已过退避期（本可试探恢复）的平台时，
  **短路优先** —— 该平台不再自动试探恢复，需用户手动重新启用。取实现最简 + 行为最可预测。

### 范围内（用户勾选四项）

1. 路由短路 —— `gateway/router/candidates.rs:186`
2. group-info 端点 —— `gateway/proxy/group_info.rs:118`（`platforms.len() != 1`）
3. 前端 GroupIcon —— `src/domains/groups/GroupIcon.tsx:8`
4. statusline 文案 —— `src/components/settings/statusline-segments.ts` 5 处 + `src/locales/*.json` 8 语言 × 6 条 key

### 范围外

- [ ] 不改 `PlatformStatus` 枚举 / DB schema / 任何 Tauri command 签名
- [ ] 不改 `expires_at` / `disable_during_peak` / 熔断的既有判定逻辑
- [ ] 不改路由排序 / 分桶 / 熔断回退（阶段 1~6）

### 已知约束

- [ ] Rust 两处判定必须共用同一个 helper，禁抄第二份（口径漂移 = 前后端行为对不上）
- [ ] 前端 `Platform` 类型已有 `status: PlatformStatus`（`src/services/api/types/generated/Platform.ts:37`），无需扩字段
- [ ] locale 改动走顶层扁平 dotted key，注入保序（memory `locale-flat-key-convention`），必跑 `check-i18n`

## 验收标准

- [ ] `gateway/router/mod.rs` 新增 `pub(crate) fn sole_platform`，实现严格等于上方判定公式，带 doc 注释说明两分支来由
- [ ] `candidates.rs:186` 的 `group_platforms.len() == 1` 改为调用 `sole_platform`，短路目标取其返回值
- [ ] `group_info.rs:118` 的 `platforms.len() != 1` 改为调用同一 `sole_platform`，`platform` 取其返回值
- [ ] 现有 3 条单平台契约测试（`test_candidates.rs:65/92/114`）**全部不改且仍通过**
- [ ] 新增 ≥3 条测试：① 3 平台仅 1 enabled → 走短路 ② 2 enabled + 1 auto_disabled → 不短路走正常路径 ③ 0 enabled 且总数 >1 → 不短路
- [ ] `test_group_info.rs` 新增 1 条：多平台仅 1 enabled → `applicable == true`
- [ ] `GroupIcon.tsx` 按同口径取 single（总数 1 或唯一 enabled），无 single 时回退首字文字框不变
- [ ] `statusline-segments.ts` 5 处「仅单平台分组」描述 → 「仅单启用平台分组」口径措辞
- [ ] `src/locales/*.json` 8 语言各 6 条 `statusline.seg.group-*` desc 同步，`node scripts/check-i18n.mjs` 零缺失
- [ ] 门禁全绿：`cargo test --workspace` / `cargo clippy --workspace`（无新增 warning）/ `npx tsc --noEmit` / `yarn test`

## 索引

- [ ] 详细设计: [design.md](design.md)
- [ ] 任务/子任务/调度: task.json (`skein subtask list single-enabled-platform-shortcut`)
