# PRD — MITM 解密白名单导入默认按钮

## 背景
`seed_default_whitelist_if_empty`（schema_late.rs:475）仅表空时填默认 37 条规则。用户已有自定义白名单 → 表非空 → seed 不跑 → 无法补回默认规则。需 UI 按钮「导入默认白名单」手动触发，只添加缺失项，去重。

## 用户决策（grill 已确认）
- **导入范围**: 仅静态默认规则 37 条（Claude 3 + OpenAI 34，`DEFAULT_RULES` 常量）。**不含动态平台 base_url host**（那是 seed 按 D6 按平台补，非「默认白名单」）。
- **只添加不覆盖**: 已有条目跳过，不删不改现有。
- **去重**: DB 层 `INSERT OR IGNORE`（host_pattern + rule_type 唯一索引）。

## 目标
- 提取 `DEFAULT_RULES` 为 `whitelist.rs` pub 常量（域归属 + schema seed + 导入 command 共用单源）
- 新 command `mitm_whitelist_import_defaults` 遍历 37 条 → INSERT OR IGNORE → 返 `(imported, skipped)` 计数
- 前端 MitmConfig 白名单区加「导入默认白名单」按钮 + toast 反馈（导入 N 条，M 条已存在）

## 产出

### D1 — DEFAULT_RULES 提取为 pub 常量
- `whitelist.rs` 加 `pub const DEFAULT_RULES: &[(&str, &str)] = &[...]`（37 条，逐条搬自 schema_late.rs:486-533 局部 const）
- `schema_late.rs::seed_default_whitelist_if_empty` 删局部 const，改引用 `crate::gateway::mitm::whitelist::DEFAULT_RULES`
- **零数据变更**（纯搬运，37 条字面不变）

### D2 — 导入 command
- `commands/mitm.rs` 加 `mitm_whitelist_import_defaults(db) -> (usize, usize)`：
  - 遍历 `whitelist::DEFAULT_RULES` → `INSERT OR IGNORE INTO mitm_whitelist (host_pattern, rule_type, enabled, source, created_at) VALUES (?, ?, 1, 'default', ?)`
  - 统计 `inserted`（changes() == 1）/ `skipped`（changes() == 0）
  - `source = 'default'`（与 seed 一致，便于后续按来源筛/清）
  - **禁删改现有条目**（只 INSERT OR IGNORE）
- `startup.rs` 注册 command

### D3 — 前端按钮 + 跨层契约
- `services/api/mitm.ts` 加 `importDefaults(): Promise<{ imported: number; skipped: number }>`
- `MitmConfig.tsx` 白名单区（标题旁，L301 区）加「导入默认白名单」按钮：
  - 调 `mitmApi.importDefaults()` → refresh → toast `t("mitm.importDefaultsDone", "已导入 {{imported}} 条默认规则（{{skipped}} 条已存在跳过）")`
  - busy 期间禁用按钮（复用 setBusy）
- 8 locale 同步补 `mitm.importDefaults`（按钮 label）+ `mitm.importDefaultsDone`（toast，带 {{imported}}/{{skipped}} 插值）

## 验证
- [ ] `cargo test whitelist`（含 DEFAULT_RULES 提取后 seed 逻辑不回归）
- [ ] 新增 `cargo test import_defaults`：mock DB 已有 1 条默认 + 1 条自定义 → import → 仅补 36 条默认缺失，自定义不动，返 (36, 1)
- [ ] `cargo clippy` 0 warning
- [ ] `yarn build` 绿 + `node scripts/check-i18n.mjs` exit 0（mitm.importDefaults + importDefaultsDone 8 locale 全补）
- [ ] 跨层契约对齐（Rust command 返 (usize, usize) ↔ TS {imported, skipped} ↔ serde）

## 非目标
- ❌ 动态平台 base_url host 导入（用户决策仅静态 37 条）
- ❌ 删除/覆盖现有白名单（只添加）
- ❌ 导入预览确认弹窗（直接导入，toast 反馈即可 — 37 条小量 + 幂等可重复点）
- ❌ 按 source 清理默认规则（另起 task）

## grill 自审 trace
- 轴 A 目标 ✓ 封闭（导入默认 37 条按钮 + 去重）
- 轴 B 产出 ✓ 可验收（test import_defaults + i18n + build）
- 轴 C 验证 ✓ 可执行断言（cargo test import_defaults 断言 (36,1) + check-i18n）
- 轴 D 资源 ✓ 7 文件列明（whitelist.rs / schema_late.rs / commands/mitm.rs / startup.rs / mitm.ts / MitmConfig.tsx / 8 locale）
- 轴 E 依赖 ✓ 单 subtask（DEFAULT_RULES 提取 → command → UI 顺序耦合，单 agent 一次做）
- 轴 F 失败 ✓ INSERT OR IGNORE 幂等（重复点安全）
- 轴 G 检查点 ✓ source='default' + 仅静态决策已 grill 用户确认
