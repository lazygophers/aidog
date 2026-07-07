---
updated: 2026-07-07
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# Locale 标签跨层一致性 (zh-Hans BCP47 script)

何时被读: 改 i18n locale 枚举 / platform-presets.json 的 name|desc locale key / 用户设置持久化 / Claude CLI language 字段时
谁读: main / sub-agent
不遵守的代价: locale 标签漂移 → presets name/desc 查 key 返 undefined 回退英文 / 旧用户升级后语言变英文 / 误统一 Claude CLI language 破坏外部命令。

---

## 应用 i18n locale 标签 = BCP 47 script 子标签 (MUST)

- **MUST `zh-Hans`** (script 子标签), **禁 `zh-CN`** (region 子标签) 作为应用 i18n locale。
- rename 历史: `07-06-locale-zh-hans-rename` 前 i18next 用 `zh-CN`, rename 后统一 `zh-Hans`。
  rename 前 `LOCALE_TO_DEFAULTS` 桥接映射 (i18next `zh-CN` ↔ presets `zh-Hans`) 已删 — **两端一致后直接用 i18next locale 作 DefaultsLocale 查 name/desc**, 禁恢复桥接层 (`defaults.ts:103-105`)。

## 三层一致 (MUST)

应用 i18n locale 标签跨三层必须**字面同一集合**:

1. **i18next** (`src/locales/index.ts`): `Locale` 类型 + `ALL_LOCALES` 数组 — 规范源。
2. **presets JSON** (`src-tauri/defaults/platform-presets.json` 每个 protocol 的 `name` / `desc` object key)。
3. **DefaultsLocale** (`src/domains/platforms/defaults.ts:9`) — 查 presets name/desc 的 locale key 类型。

- 新增 locale: 三层同步加, **禁只改一层** (i18next 加了但 presets 没 key → name 查 undefined 回退 en-US; 加 presets key 但 i18next 没枚举 → 切不到)。
- 默认 + fallback: `lng: "zh-Hans"`, `fallbackLng: "en-US"` 同步打包保证首屏 `t()` 立即可用; 其余 5 语言按需 dynamic import (`ensureLocaleLoaded`)。

## 持久化迁移 (MUST, 单向)

- `src/context/AppContext.tsx:98` 启动读用户设置时: `raw.locale === "zh-CN"` → 改写 `"zh-Hans"`。
- **单向兼容**: 旧用户持久化存的 `zh-CN` 自动迁 `zh-Hans`; 反向不禁 (新版本不写 `zh-CN`)。
- 改 locale 枚举时禁删此迁移行, 否则 07-06 前老用户升级后 locale 失效落 fallback en-US。

## 多 locale 命名空间共存, 禁统一 (MUST NOT)

应用内存在 **4 套独立 locale 命名空间**, 各服务不同消费者, 标签约定不同是有意设计, **禁强行统一**:

| 命名空间 | 标签 | 位置 | 消费者 |
|---|---|---|---|
| 前端 i18next (规范源) | `zh-Hans` | `src/locales/index.ts` | React UI |
| presets JSON name/desc key | `zh-Hans` | `src-tauri/defaults/platform-presets.json` | 协议显示名/描述 (跟前端) |
| 后端 i18n `Lang` 枚举 | `ZhCn` 变体 | `src-tauri/src/gateway/i18n.rs` | 后端通知/文案 |
| Claude CLI language | `zh-CN` | `src/services/claude-settings-schema.ts` + `src-tauri/defaults/settings.json` bundled `language` | 写入 `~/.claude/settings.json` 供 Claude CLI 读 |

- **后端 `Lang::from_locale` 兼容多种标签归一**: `zh-CN`/`zh_CN`/`zh-Hans`/`zh_Hans`/`zh` 全 → `Lang::ZhCn` (`i18n.rs:20`)。即后端入口容错吃下前端 `zh-Hans` 与 Claude CLI `zh-CN` 两种写法, 这是**入口归一层**, 不是规范源标签放宽。
- **Claude CLI language MUST 保持 `zh-CN`**: Claude CLI 只认它自己的 region 命名; 误统一成 `zh-Hans` → Claude CLI 不认回退默认。
- 改任一命名空间禁波及其他三套; **禁"顺手统一"** (如改前端 locale 时顺手改 claude-settings-schema 的 LANGUAGE_OPTIONS)。

## 测试 fixture / 文档 URL (合法残留, 非命名空间)

- 测试用 `zh-CN` fixture (`test_sync_settings.rs` / `test_apply.rs` / `test_selection.rs` / `test_collect.rs` / `test_i18n.rs`) 测旧数据导入导出兼容, 合法。
- `code.claude.com/docs/zh-CN/...` 是文档站 URL path 片段 (非 locale 标签), 合法无关。

## RTL

`ar-SA` 是唯一 RTL locale (`RTL_LOCALES`, `index.ts:28`); `isRTL()` 查表。新增 RTL 语言同步加此数组。

## 验收基准 (可复用)

- [ ] `ALL_LOCALES` 集合 == presets JSON 任一 protocol 的 `name` object key 集合 == `DefaultsLocale` 枚举
- [ ] grep `zh-CN` 仅命中合法点: `AppContext.tsx:98` (迁移行) + `defaults.ts:104` (历史注释) + Claude CLI 命名空间 (`claude-settings-schema.ts` + `settings.json`) + 后端 i18n (`i18n.rs` from_locale 兼容 + `test_i18n.rs`) + 导入导出测试 fixture + 文档 URL path 片段
- [ ] 旧用户 `localStorage` 存 `locale: "zh-CN"` 启动后变 `zh-Hans`, UI 中文
- [ ] Claude CLI language 选项含 `zh-CN` 不含 `zh-Hans`

## 验证命令

```bash
# zh-CN 残留审计 (合法点见上 "测试 fixture / 文档 URL" + 4 命名空间表; 出现新点须判归属)
grep -rn "zh-CN" src/ src-tauri/ | grep -v "lang\.zh-Hans"

# presets name/desc locale key 集合 (取一个 protocol 比对)
grep -A10 '"name": {' src-tauri/defaults/platform-presets.json | grep -oE '"(zh-Hans|en-US|ar-SA|fr-FR|de-DE|ru-RU|ja-JP|es-ES)"' | sort -u

# i18next 规范源
grep -E '"zh-Hans"|"en-US"|"ar-SA"|"fr-FR"|"de-DE"|"ru-RU"|"ja-JP"|"es-ES"' src/locales/index.ts
```

## 关联

- [backend/index.md](../backend/index.md) — presets JSON (后端 bundled, locale key 跟随前端规范源)
- `src/locales/index.ts` (i18next 规范源) / `src/context/AppContext.tsx:98` (持久化迁移) / `src/services/claude-settings-schema.ts:31` (外部命名空间隔离)
