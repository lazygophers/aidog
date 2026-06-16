# i18n labelKey 盲区修复: t(变量) 路径 key 缺失 + check-i18n D 检查

## 背景

i18n-coverage-hardening (已 archive) 声称根治裸 key, 但用户报告菜单仍显示 key (如 `appSettings.systemTab`)。复发根因 = check-i18n.mjs 第三类盲区。

## 根因

`scripts/check-i18n.mjs` 扫描规则:
- staticRe = `\bt\(\s*['"]([^'"]+)['"]` — 要求 `t(` 后跟引号字面量
- dynRe = `\bt\(\s*\`([^\`]+)\`` — 要求反引号模板

**两者都漏 `t(变量)` 形式**:
```tsx
// App.tsx NAV_ITEMS 定义 labelKey 字面量
{ id: "settings/system", labelKey: "appSettings.systemTab", group: "nav.settingsGroup.general" }
// Sidebar.tsx 消费
{t(item.labelKey)}      // ← t(变量), 不被任何正则扫到
{t(g.key)}              // ← 同
{t(section.labelKey)}   // ← Settings.tsx / CodexSettings.tsx / SectionAnchorNav.tsx 同模式
```

labelKey 字面量是 i18n key 的**数据源**, 但从未被检查覆盖 → 新增 tab 时只改 App.tsx, 忘补 locale → 裸 key 显示。i18n-coverage-hardening 任务修了 t("lit") / t(`tpl`) 两路, 漏了 t(变量) 第三路。

## 缺失清单 (诊断已确认)

### A. labelKey 字面量缺 locale key (5 key × 8 locale = 40 条)
- `appSettings.systemTab` (用户报的, "系统")
- `appSettings.claudeTab` ("Claude")
- `appSettings.codexTab` ("Codex")
- `appSettings.pricingTab` ("定价")
- `appSettings.trayTab` ("托盘")

来源: `src/App.tsx` NAV_ITEMS (9 项, locale 仅 4 项 middleware/notifications/popover/scheduling)

### B. ja-JP 空值
- `settings.perm.noRulesPrefix` = "" → 应 "ルールなし" (其他 locale 已译)

### C. 误报 (非 i18n, D 检查须排除)
- `APIKEY.FUN` / `CTok.ai` = `src/pages/Platforms.tsx` 平台预设 `label` 属性 = 品牌名, 非 i18n key

## 修复方案

### 1. 补 locale key (worktree 内)
- 5 个 appSettings.*Tab × 8 locale (品牌 Claude/Codex 保留不译, 仅 tab 名本地化)
- ja-JP settings.perm.noRulesPrefix 补值
- 排序保持字母序 (与现有约定一致)

### 2. 强化 check-i18n.mjs — 新增 D 检查
**D. labelKey/group 属性字面量覆盖**: 扫 src 全 .ts/.tsx, 提取 `labelKey:` / `group:` 属性赋值的点号字符串值 (排除纯字母无点的), 每 key 必须在 8 locale 存在。
- 属性集限定 `labelKey` + `group` (明确 i18n 语义, 排除 Platforms 品牌 `label`)
- 排除白名单: 值含非 key 字符 (大写连续如 `APIKEY.FUN` / 域名 `CTok.ai`) — 启发: 全大写段或含非 `[a-z0-9_.]` 跳过
- exit 1 当缺失

### 3. spec 更新
`.trellis/spec/frontend/conventions.md` i18n 章节: 新增 t(变量) 路径规则 + labelKey/group 属性值必须 8 locale 同步。

### 4. memory
`frontend-i18n-coverage.md` 加第三类盲区 (t(变量)) + D 检查防线。

## 验证

- `node scripts/check-i18n.mjs` exit 0 (A/B/C/D 全过)
- `npx tsc --noEmit` 无错
- 8 locale JSON 合法
- 手动确认: 切 ar-SA/ja-JU 等 locale, AppSettings tab 名显示翻译非 key

## 不做

- 不动 Platforms.tsx 品牌 label (APIKEY.FUN/CTok.ai 是预设品牌)
- 不重构 NAV_ITEMS 结构 (仅补 key)
- 不动其他 locale 已有 key
