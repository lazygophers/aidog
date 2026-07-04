# PRD — MITM 解密白名单 一键清空/搜索/URL 命中测试

## 背景
白名单 UI 已有增/删/toggle/导入默认。用户需补 3 功能：一键清空、搜索过滤、URL 命中测试。

## 用户决策（grill 已确认）
- **清空范围**: 全删（default + user，DELETE FROM mitm_whitelist）。可重新「导入默认白名单」恢复 37 条静态默认。
- **URL 命中粒度**: 返命中规则列表 `[{host_pattern, rule_type}]`，前端展示哪些规则命中（透明）。

## 目标
- D1 一键清空 command + 前端 confirm 弹窗
- D2 搜索过滤（前端纯 filter，无后端）
- D3 URL 命中测试 command（解析 host + 遍历 enabled 规则 matches_rule）+ 前端输入框 + 结果展示

## 产出

### D1 — 一键清空
- `commands/mitm.rs` 加 `mitm_whitelist_clear(db) -> Result<usize, String>`：`DELETE FROM mitm_whitelist` 返删除行数
- `startup.rs` 注册
- `MitmConfig.tsx` 白名单区加「清空」按钮（与「导入默认」并列）：
  - 点击 → 弹 React confirm modal（禁 window.confirm，破坏 Tauri）「确认清空全部 N 条白名单？此操作不可撤销，但可重新导入默认规则」
  - 确认 → 调 command → refresh → toast `t("mitm.clearDone","已清空 {{n}} 条白名单规则")`
  - busy 期间禁用

### D2 — 搜索过滤（前端纯）
- `MitmConfig.tsx` 白名单列表上方加搜索输入框：
  - `useState<string>` search + `useMemo` filter `whitelist.filter(e => e.host_pattern.toLowerCase().includes(search.toLowerCase()))`
  - 实时过滤（无按钮），placeholder `t("mitm.searchPlaceholder","搜索规则…")`
  - 搜索命中 0 条时展示空态 `t("mitm.searchEmpty","无匹配规则")`

### D3 — URL 命中测试
- `whitelist.rs`：`matches_rule`（L62 私有）改 `pub fn`，供 command 复用（单源匹配引擎，禁 command 内联重写）
- `whitelist.rs` 加 `pub fn evaluate_host(entries: &[WhitelistEntry], host: &str) -> Vec<WhitelistEntry>`：遍历 **enabled** 条目，`matches_rule` 命中即收集（返命中规则，反映 MITM 实际行为 = 仅 enabled 生效）
- `commands/mitm.rs` 加 `mitm_whitelist_test_url(url: String, db) -> Result<Vec<MatchedRuleDto>, String>`：
  - 解析 URL 取 host（`url` crate 或手写：剥 scheme + path/port，取 host）。输入是裸 host 也接受（直接匹配）
  - `list_whitelist(db)` → filter enabled → `evaluate_host` → 映射 `MatchedRuleDto { host_pattern, rule_type }`
- `startup.rs` 注册
- `mitm.ts` 加 `clearWhitelist(): Promise<number>` + `testUrl(url): Promise<{host_pattern, rule_type}[]>`
- `MitmConfig.tsx` 白名单区加 URL 测试输入框 + 测试按钮：
  - 输入 URL → 调 command → 结果区展示：命中 N 条 + 规则列表（host_pattern + rule_type badge），或 `t("mitm.testNoHit","未命中任何规则")`
  - 输入空禁用测试按钮

### i18n（8 locale 同步）
新 key：`mitm.clear` / `mitm.clearConfirm` / `mitm.clearDone`（{{n}}）/ `mitm.searchPlaceholder` / `mitm.searchEmpty` / `mitm.testUrlLabel` / `mitm.testUrlPlaceholder` / `mitm.testUrlBtn` / `mitm.testUrlHit`（{{n}}）/ `mitm.testNoHit` / `mitm.cancel`（确认弹窗取消）

## 验证
- [ ] `cargo test whitelist`（含 matches_rule pub 后不回归 + evaluate_host 新测）
- [ ] 新增 `cargo test evaluate_host`：mock entries（domain/suffix/keyword/ipcidr 各 1）→ evaluate_host(api.anthropic.com) 命中 suffix anthropic.com → 返正确子集
- [ ] 新增 `cargo test whitelist_clear`：mock N 条 → clear → 返 N + 表空
- [ ] `cargo clippy` 0 warning
- [ ] `yarn build` 绿 + `node scripts/check-i18n.mjs` exit 0（新 key 8 locale 全补）
- [ ] 跨层契约对齐（Rust Vec<MatchedRuleDto> ↔ TS {host_pattern, rule_type}[]）

## 非目标
- ❌ 清空按 source 筛选（用户决策全删）
- ❌ URL 命中测试 disabled 规则（仅 enabled 反映 MITM 实际行为）
- ❌ 搜索后端化（前端 filter 够，白名单小量 ≤几百条）
- ❌ 清空 undo（可重新导入默认恢复）

## grill 自审 trace
- 轴 A 目标 ✓ 3 功能封闭
- 轴 B 产出 ✓ 可验收（cargo test evaluate_host/clear + build + i18n）
- 轴 C 验证 ✓ 可执行断言
- 轴 D 资源 ✓ 文件列明（whitelist.rs / commands/mitm.rs / startup.rs / mitm.ts / MitmConfig.tsx / 8 locale）
- 轴 E 依赖 ✓ 3 功能文件交集大（同改 MitmConfig + commands）→ 单 subtask 一次做，禁拆并行（同文件冲突）
- 轴 F 失败 ✓ confirm 弹窗防误清空；evaluate_host 复用 matches_rule 单源
- 轴 G 检查点 ✓ 清空范围 + URL 粒度已 grill 用户确认
