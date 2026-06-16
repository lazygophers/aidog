# 组级卸载: 卸载整个分组的 skills

## Goal

Skills 页组头加「卸载整组」入口，一键卸载某 source 分组内所有 skill。破坏性操作，二次确认 modal（与现有「卸载全部」一致模式）。复用现有 `uninstall`（含 fs 兜底删第三方 symlink）。

## 设计

**后端** `gateway/skills.rs::uninstall_group(group_source: Option<&str>, scope, proxy)`:
- list_installed 过滤 source 匹配（None 组 = source=None 的 skill）。
- 逐个调 `uninstall(name, scope, proxy)`（复用，含 npx remove + fs 兜底）。
- 汇总成功/失败/skip（空组 skip）+ invalidate(scope)。
- 返回 SkillsOpResult（stdout "ok/total"，stderr 聚合失败明细）。

**lib.rs** command `skills_uninstall_group(group_source: Option<String>, scope)` + invoke_handler 注册。

**前端** Skills.tsx:
- 组头加 btn-danger 小按钮「卸载整组」（disabled when busyKey != null）。
- 点击 → setUninstallGroupTarget({source, label, count})（新 state）。
- 二次确认 modal（createPortal，与 confirmUninstall 同模式）：显示组名 + skill 计数 + 警告。
- 确认 → skillsApi.uninstallGroup(source, scope) → busyKey `group:uninstall:<key>` → 成功 refreshInstalled + setMessage；失败弹错。
- busyKey = `__uninstall_group_<key>__`。

**api.ts**: `skillsApi.uninstallGroup(groupSource, scope)` → invoke skills_uninstall_group。

**i18n**: `skills.uninstallGroup`（「卸载整组」）+ `skills.uninstallGroupConfirm`（「将卸载 {{group}} 下 {{count}} 个 skill，不可恢复」）× 8 locale。

## Acceptance

- [ ] uninstall_group 后端 + 单测（mock 不真跑 npx 写，靠类型 + 编译；过滤逻辑靠 list_installed 真值）。
- [ ] skills_uninstall_group command + invoke_handler 注册。
- [ ] 组头「卸载整组」按钮 + 二次确认 modal。
- [ ] 第三方 symlink skill（无 source）归 None 组可整组卸载（fs 兜底生效）。
- [ ] cargo clippy 0 项目 warning；cargo test 全过；yarn build OK；check-i18n 零缺失。

## Out of Scope

- 组级卸载不区分 agent（卸载 = 删规范存储 + 所有 agent symlink，与单一卸载语义一致）。
- 不改现有单一卸载 / 全部卸载。
