---
inclusion: auto
name: zh-hans-literal-sync
title: 应用 locale 标签字面同集（zh-Hans × BCP47）
layer: core
category: i18n
keywords: [locale,i18n,zh-Hans,BCP47,script,preset,sync]
source: -
authored-by: skein-memory
created: 1722556801
---
---
# 应用 locale 标签字面同集（zh-Hans × BCP47）

何时被读: 改 i18n locale 枚举 / platform-presets.json locale key / 用户设置持久化 / UI locale 切换时

不遵守的代价: locale 标签漂移 → presets name/desc 查 key 返 undefined 回退英文 / 旧用户升级后语言变英文 / 跨层命名空间混乱

---

## MUST 硬约束

- **应用 i18n 使用 BCP47 script 子标签 `zh-Hans`（非 region 子标签 `zh-CN`）**
  - rename 历史：07-06-locale-zh-hans-rename 前用 `zh-CN`，rename 后统一 `zh-Hans`
  - 两端一致后直接用 i18next locale 作 DefaultsLocale 查 presets name/desc，禁恢复桥接层

- **三层字面同集（禁任何一层单独改）**
  - i18next 规范源 (`src/locales/index.ts`：`Locale` 类型 + `ALL_LOCALES`)
  - presets JSON (`src-tauri/defaults/platform-presets.json` 每个 protocol 的 `name`/`desc` object key)
  - DefaultsLocale (`src/domains/platforms/defaults.ts` 查 presets 的 locale key 类型)
  - 新增 locale：三层同步加，禁只改一层（i18next 加但 presets 没 key → name 查 undefined；加 presets 但 i18next 没枚举 → 切不到）

- **持久化兼容迁移单向**
  - `src/context/AppContext.tsx` 启动读：`raw.locale === "zh-CN"` → 改写 `"zh-Hans"`
  - 旧用户持久化存的 `zh-CN` 自动迁 `zh-Hans`；反向禁（新版本不写 `zh-CN`）
  - 改 locale 枚举禁删此迁移行，否则 07-06 前老用户升级后 locale 失效落 fallback en-US

## 禁（多 locale 命名空间统一）

❌ **禁强行统一 4 套独立 locale 命名空间，各有约定**：

| 命名空间 | 标签 | 位置 | 消费者 |
|---|---|---|---|
| 前端 i18next（规范源） | `zh-Hans` | src/locales/index.ts | React UI |
| presets JSON name/desc key | `zh-Hans` | src-tauri/defaults/platform-presets.json | 协议显示名/描述 |
| 后端 i18n Lang 枚举 | ZhCn 变体 | src-tauri/crates/aidog_core/src/gateway/i18n.rs | 后端通知/文案 |
| **Claude CLI language（外部命名空间）** | **`zh-CN`** | **src/services/claude-settings-schema.ts** | **写入 `~/.claude/settings.json` 供 Claude CLI 读** |

✅ **正确做法**：
- 后端 `Lang::from_locale` 兼容多种标签归一：`zh-CN`/`zh_CN`/`zh-Hans`/`zh_Hans`/`zh` 全 → `Lang::ZhCn` (i18n.rs:20)
  这是入口归一层，不是规范源放宽
- **Claude CLI 命名空间 MUST 保持 `zh-CN`**：Claude CLI 只认自己的 region 命名；误统一成 `zh-Hans` → Claude CLI 不认回退默认
- 改任一命名空间禁波及其他三套；禁"顺手统一"（如改前端 locale 时顺手改 claude-settings-schema 的 LANGUAGE_OPTIONS）

## 验收

- [ ] `ALL_LOCALES` 集合 == presets JSON 任一 protocol 的 `name` object key 集合 == `DefaultsLocale` 枚举
- [ ] `zh-CN` 残留仅限合法点：AppContext.tsx 迁移行 + 后端 i18n + Claude CLI 命名空间 + 测试 fixture + 文档 URL path
- [ ] 旧用户 localStorage 存 `locale: "zh-CN"` 启动后变 `zh-Hans`，UI 中文
- [ ] Claude CLI language 选项含 `zh-CN` 不含 `zh-Hans`

## 关联

[[i18n-key-sync-8lang]] [[locale-deadkey-cleanup-ownership]]
