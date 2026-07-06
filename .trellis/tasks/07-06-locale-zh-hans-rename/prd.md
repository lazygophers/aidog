# locale code zh-CN → zh-Hans 全量 rename

## Goal
应用 i18n 默认 locale code 从 `zh-CN` (region) 改为 `zh-Hans` (script), 符合 BCP 47 现代规范。**全量 rename**: 文件名 + 全仓引用 + 已 persisted 用户偏好迁移, 一次到位。

## 背景
- `zh-CN` = region 子标签 (China), `zh-Hans` = script 子标签 (Simplified Chinese)。BCP 47 现代规范优先 script 子标签描述书写系统, 不绑定国家。i18next / Chrome / MDN均倾向 zh-Hans。
- 项目当前 8 locale, zh-CN 是默认 (`lng: "zh-CN"`) + 同步打包 (首屏可用)。

## 改动 (全量 rename, 单 subtask)

### 文件 rename
- `src/locales/zh-CN.json` → `src/locales/zh-Hans.json` (`git mv` 保留历史)

### 引用更新 (grep 验零遗漏: `grep -rn "zh-CN" src/`)
- `src/locales/index.ts`:
  - `import zhCN` → `import zhHans` + from `"./zh-Hans.json"`
  - `Locale` union type: `"zh-CN"` → `"zh-Hans"`
  - `ALL_LOCALES` 数组: `"zh-CN"` → `"zh-Hans"`
  - `resources`: key `"zh-CN"` → `"zh-Hans"`
  - `i18n.use(...).init({ lng: "zh-CN" ... })` → `"zh-Hans"`
  - 注释 `默认语言 (zh-CN)` → `(zh-Hans)`
- `src/context/AppContext.tsx:96`: `raw.locale ?? "zh-CN"` → `?? "zh-Hans"`
- `src/test/render.tsx`: 3 处 (`resources` key / `lng` / `fallbackLng`)
- `CLAUDE.md` (项目): UI/i18n 节 `8 种语言（zh-CN / en-US / ...）` → `zh-Hans / en-US / ...`

### Persisted 用户偏好迁移 (关键, 不做老用户掉语言)
- `src/context/AppContext.tsx` 读 `localStorage["locale"]` 时: 若值为 `"zh-CN"` → 当作 `"zh-Hans"` (透明迁移); 顺便持久化回写新值
- 实现: 加一行 `if (raw.locale === "zh-CN") raw.locale = "zh-Hans";` 在 default 前面

### 不动 (重要边界)
- `src/services/claude-settings-schema.ts`: 这是 **Claude Code 自身** 的 settings schema (`language` 字段值域), 非本项目 i18n。`zh-CN` 是 Claude Code 接受的合法值, **保留不动**。
- `src/locales/index.ts:34` 注释提及「默认语言 + fallback (en-US) 同步打包」的打包逻辑保持, 仅 code 改名
- `docs/` rspress 文档站: 用 `zh` (无 region/script), 独立体系, 不动
- 8 个 `README.xx.md`: 文件名用 region code (README.fr.md / README.en.md), 是文档多语言约定, 与 app locale 解耦, 不动

## Acceptance
- [ ] `src/locales/zh-CN.json` 已 `git mv` 为 `zh-Hans.json`
- [ ] `grep -rn "zh-CN" src/` 仅剩 claude-settings-schema.ts (Claude Code 自身配置, 不动)
- [ ] i18n init `lng: "zh-Hans"`, fallback 仍 `en-US`
- [ ] localStorage 老值 `"zh-CN"` 透明迁移到 `"zh-Hans"` (已 persisted 用户不掉语言)
- [ ] CLAUDE.md 项目文件 UI/i18n 节更新
- [ ] `yarn build` + 全部 `yarn test` 通过 (test/render.tsx 同步改)
- [ ] `node scripts/check-i18n.mjs` 零缺失 (8 locale 齐全)
- [ ] dev 启动后 UI 显示中文 (zh-Hans 资源正确加载, 不 fallback 英文)

## Out of Scope
- Claude Code 自身 settings schema (`claude-settings-schema.ts`)
- rspress `docs/` 多语言站
- README.xx.md 文件命名
- 其它 7 个 locale (en-US / ar-SA / fr-FR / de-DE / ru-RU / ja-JP / es-ES) 改名 (无需求, YAGNI)

## Technical Notes
- i18next 资源 key 与 `lng` 必须严格一致, 否则 fallback 到 `fallbackLng` (en-US) → 中文用户看英文。改完必须 dev 验证。
- ALL_LOCALES 是 Sidebar.tsx 语言选择器的数据源, 改完选择器自动显示 zh-Hans 选项 (UI label 用 i18n 自描述 key, 与 code 解耦)
- `git mv` 而非删+建, 保留 git blame 历史

## 依赖
- 无 (与 defaults-sync task 文件集不相交, 但因都改 8 locale json, **必须串行**: 等 defaults-sync merge 后再 start, 避免 worktree merge 冲突)
