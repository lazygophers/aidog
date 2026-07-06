# platform-presets protocols 多语言 name/desc + 搜索 (含拼音)

## Goal
为 `src-tauri/defaults/platform-presets.json` 每个 protocol 项加多语言 `name` + `desc` (8 locale), 让前端协议选择/搜索 UI 按当前 locale 显示本地化文案。**JSON 成单一真值源**, constants `PROTOCOLS.label` 改从 JSON 派生, 消除双源漂移。搜索+拼音基础 (`SearchableProtocolSelect` + `pinyinMatch`) 已存在, 本 task 接入多语言数据即可生效 (pinyinMatch 自动匹配新 zh-Hans name)。

## 决策 (brainstorm 已确认)
- ✅ 多语言形态: **JSON 内嵌** — `name`/`desc` = `{zh-Hans, en-US, ar-SA, fr-FR, de-DE, ru-RU, ja-JP, es-ES}` 8 locale
- ✅ 真值源归一: **JSON 成单一真值源**, `constants PROTOCOLS.label` 改派生 (运行时查 JSON), 消双源
- ✅ 翻译范围: **8 locale 全手翻** (60 protocols × 8 locale × 2 字段 = 960 strings)
- ✅ desc: name + desc 都要 (name 短标签 / desc 一句话描述)
- ✅ keywords: 不翻译, 保留现有中英混合搜索 hint (用户不可见, 仅匹配用, YAGNI)

## JSON Schema (新增字段)
```json
"anthropic": {
  "client_type": "claude_code",
  "endpoints": { ... },
  "models": { ... },
  "model_list": { ... },
  "name": {
    "en-US": "Anthropic",
    "zh-Hans": "Anthropic",
    "ar-SA": "Anthropic",
    "fr-FR": "Anthropic",
    "de-DE": "Anthropic",
    "ru-RU": "Anthropic",
    "ja-JP": "Anthropic",
    "es-ES": "Anthropic"
  },
  "desc": {
    "en-US": "Anthropic official API for Claude models",
    "zh-Hans": "Anthropic 官方 API, Claude 系列模型",
    "ar-SA": "...",
    "fr-FR": "...",
    "de-DE": "...",
    "ru-RU": "...",
    "ja-JP": "...",
    "es-ES": "..."
  }
}
```

## 改动范围

### A. platform-presets.json 数据 (核心, 体量大)
- 60 protocols × 8 locale × 2 字段 = 960 strings 手翻写入 JSON
- en-US + zh-Hans: 原创翻译 (来自现有 constants label + 公开知识)
- ar/fr/de/ru/ja/es: 从 en-US 翻译
- 子任务拆分 (建议 subtask fan-out):
  - subtask A1: schema 落地 + en-US + zh-Hans 原创写入 (60×2×2=240 strings)
  - subtask A2-A7: 6 locale 翻译并行 (每 subtask 1 locale × 60 protocols × 2 字段 = 120 strings, 6 个并发上限 2 串行批次)

### B. Rust 后端 (defaults.rs)
- `ProtocolPreset` 反序列化 struct 加 `name` / `desc` 字段 (`Option<HashMap<String, String>>` 或专用 I18nString 类型)
- `get_defaults_json` Tauri command 透传新字段到前端 (serde 自动)
- 不改 sync 逻辑 (jsDelivr/raw URL 不变, 同步的 JSON 自带新字段)

### C. 前端 constants.ts PROTOCOLS.label 派生
- `constants.ts`: `PROTOCOLS` 数组的 `label` 字段从硬编码改为运行时读 JSON protocol.name[currentLocale]
- 但 `PROTOCOLS` 是模块级 const, 异步加载 JSON 后才能填充 → 改为 async loader 或 lazy getter
- 影响调用点: `SearchableProtocolSelect` / `matchPlatform` / `platformPaste` / `ccswitchMatch` / `usePlatformsState` 等
- **关键约束** (来自 CLAUDE.md): defaults.ts 4 函数全 async + docPromise 单次 RPC 缓存, 所有 caller 必须 await — 改 label 派生须遵循此模式
- 简化: 新增 `getProtocolLabel(proto, locale)` async helper, PROTOCOLS.label 保留为 fallback 英文 (硬编码英文兜底), 选择器渲染时优先 await getProtocolLabel

### D. i18next 接入 (前端)
- 协议选择器渲染: `useTranslation()` 拿当前 locale → 读 JSON protocol.name[locale]
- fallback 链: locale 缺失 → en-US → protocol key (如 "anthropic")
- 拼音搜索: pinyinMatch 自动应用于新 zh-Hans name (无需改 pinyinMatch, 数据换了自动生效)

### E. 测试 + 验证
- `scripts/check-i18n.mjs`: 加 protocol name/desc 8 locale 完整性校验 (每 protocol 8 locale 都有 name+desc, 无缺)
- pinyin 搜索测试: zh-Hans name 拼音匹配 (如 "百炼" → "bailian" 可搜)
- cargo test (Rust serde 反序列化) / yarn build / yarn test (前端)

## Acceptance
- [ ] platform-presets.json 60 protocols 全部含 name + desc, 每 field 8 locale 齐全
- [ ] `python3 -c "json.load; [...check all protocols have name/desc × 8 locale]"` 零缺失
- [ ] Rust `ProtocolPreset` 反序列化新字段, cargo build/test/clippy 全绿
- [ ] 前端 constants PROTOCOLS.label 派生从 JSON (运行时查), fallback 链: locale → en-US → key
- [ ] SearchableProtocolSelect 渲染按当前 locale 显示, 切语言 (Sidebar.tsx) 后协议 label 跟随切换
- [ ] 拼音搜索: zh-Hans name 输拼音命中 (如 "bailian" 搜到 "百炼")
- [ ] `scripts/check-i18n.mjs` 通过 (含新增 protocol name/desc 校验)
- [ ] `yarn build` + `yarn test` + `cargo test --lib` 全绿
- [ ] dev 启动验证: 切 8 locale 各看协议选择器本地化 + 拼音搜索可用

## Out of Scope
- keywords 多语言翻译 (保留中英混合搜索 hint)
- 新增 protocol (仅给现有 60 个加 name/desc)
- 翻译质量审计 (本轮手翻为准, 后续 neat-freak / i18n-auditor agent 复审)
- Tauri resources / build 配置 (运行时同步, 非内嵌)
- protocol color / hosts / codingKeyPrefixes 字段迁移 (仍留 constants)

## Technical Notes
- 960 strings 工作量大, subtask fan-out 6 locale 翻译并发 (上限 2 串行批次, 每 batch 3 locale 并行)
- en-US + zh-Hans 原创优先 (其它 locale 翻译源), 须先完成 A1 再启 A2-A7
- `constants.ts PROTOCOLS.label` 改造影响多调用点, 须 grep 全部 caller (PROTOCOLS / PROTOCOL_LABELS) 确认无遗漏
- defaults.ts async 模式 (CLAUDE.md 强制): label 派生走同样 docPromise 模式, 所有 caller await
- i18next fallback 链: lng → fallbackLng(en-US) → key; JSON name[locale] 缺失时同样回退

## 依赖
- 阻塞: 与 `07-06-locale-zh-hans-rename` 文件集部分相交 (都涉及 src/locales/* + i18n 资源 + AppContext 切语言) → 须串行, locale task 先 (或后), 避免双 worktree merge 冲突
- 子任务内部依赖: A1 (en+zh 原) → A2-A7 (6 locale 翻译) → B+C+D (前后端接入) → E (测试)
