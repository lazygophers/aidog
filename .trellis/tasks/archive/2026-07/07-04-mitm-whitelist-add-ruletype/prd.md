# PRD — MITM 白名单添加规则时选匹配方式

## 背景
白名单添加 UI 仅一个 host_pattern 输入框，后端 INSERT 硬编码 `rule_type='suffix'`（commands/mitm.rs:222）。用户无法选 domain/suffix/keyword/ipcidr 匹配方式，导致非 suffix 语义规则（如精确 domain、CIDR、关键词）添加后匹配行为错误。

## 目标
添加规则时支持选 rule_type（4 种），前端选择器 + 后端用所选 rule_type 入库（替代硬编码 suffix）+ 合法值校验。

## 决策推荐（grill 待确认）
- **默认值**：suffix（当前硬编码，向后兼容；DEFAULT_RULES 多数为 suffix）
- **控件**：`<select>` 下拉，紧邻 host_pattern 输入框左侧
- **option label**：裸字符串 `domain/suffix/keyword/ipcidr`（技术常量，与现有 rule_type badge 展示一致，遵循"协议/技术常量保留原文"约定，不加 i18n）
- **后端校验**：rule_type 必须是 4 合法值之一，非法返 Err（防脏数据进 matches_rule 走不到的分支）

## 产出

### D1 — 后端（commands/mitm.rs）
- `WhitelistAddInput` 加 `pub rule_type: String`（必填，前端总传）
- `mitm_whitelist_add`：校验 `input.rule_type` ∈ {domain,suffix,keyword,ipcidr}，非法返 `Err("invalid rule_type: <x>")`；INSERT 用 `input.rule_type` 替代硬编码 `'suffix'`：
  ```rust
  "...VALUES (?1, ?2, 1, 'user', ?3)", params![pattern, rule_type, now]
  ```
- rule_type 校验单源：抽 `fn valid_rule_type(s: &str) -> Option<&'static str>`（返归一化小写常量，后续 toggle/remove 若需可复用）

### D2 — 前端契约（services/api/mitm.ts）
- `whitelistAdd(hostPattern: string, ruleType: RuleType)` —— RuleType 复用 WhitelistEntry.rule_type union `"domain"|"suffix"|"keyword"|"ipcidr"`（L15 已定义）
- invoke args: `{ input: { host_pattern: hostPattern, rule_type: ruleType } }`

### D3 — UI（MitmConfig.tsx）
- 加 `useState<RuleType>("suffix")` newRuleType
- 添加区（L349-361）input 前加 `<select>`：4 option（裸字符串值 + label），value=newRuleType，onChange setNewRuleType
- `handleAdd`（L124-134）：调 `whitelistAdd(p, newRuleType)`；添加成功后 newRuleType 不重置（保持上次选择，用户连加同类型规则方便）
- select 加 `aria-label={t("mitm.ruleTypeLabel","匹配方式")}` 或前置 inline label

### D4 — i18n（8 locale）
新 key：`mitm.ruleTypeLabel`（"匹配方式"）。option label 裸字符串不译（技术常量）。

## 验证
- [ ] `cd src-tauri && cargo test commands::mitm`（新增 valid_rule_type 校验测：4 合法通过 + 非法返 Err）
- [ ] `cargo clippy` 0 warning
- [ ] `yarn build` 绿 + `node scripts/check-i18n.mjs` exit 0（mitm.ruleTypeLabel × 8 locale）
- [ ] 跨层契约：TS RuleType union ↔ Rust WhitelistAddInput.rule_type: String + 校验
- [ ] 实测：添加 suffix/domain/keyword/ipcidr 各 1 条 → DB rule_type 列正确 + matches_rule 行为符合类型

## 非目标
- ❌ 改 matches_rule 匹配引擎（已是单源 pub fn，4 类型正确）
- ❌ 改 rule_type badge 展示（保持裸字符串）
- ❌ 编辑已有规则的 rule_type（仅新增时选；已有规则改类型需求未提）
- ❌ ipcidr 格式严格校验（matches_rule 已容错，add 时宽松接受）

## grill 自审 trace
- 轴 A 目标 ✓ 添加时选 rule_type，封闭
- 轴 B 产出 ✓ 后端校验+INSERT + 前端 select + 契约 + i18n，可验收
- 轴 C 验证 ✓ cargo test + clippy + build + i18n + 实测
- 轴 D 资源 ✓ commands/mitm.rs + mitm.ts + MitmConfig.tsx + 8 locale，单 subtask（同 MitmConfig 文件禁拆并行）
- 轴 E 依赖 ✓ 单文件集，单 subtask 一次做
- 轴 F 失败 ✓ 后端校验防脏数据；rule_type 默认 suffix 向后兼容
- 轴 G 检查点 ✓ 决策推荐明确（默认/控件/label/校验），grill 一次性确认
