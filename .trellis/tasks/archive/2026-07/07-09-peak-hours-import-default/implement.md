# Implement Plan — peak_hours 导入默认配置按钮

## subtask 拆分 (顺序，单前端面，DAG 无并行)

### ST1: defaults.ts 加 getDefaultPeakHours
- 文件: src/domains/platforms/defaults.ts
- 改动:
  - 在 getDefaultModelList (line 122) 后加 `getDefaultPeakHours(protocol: Protocol): Promise<PeakWindow[]>`
  - 实现: `const doc = await loadDoc(); const entry = doc.protocols[protocol]; const list = entry?.peak_hours ?? []; return list.map(w => ({ ...w, days_of_week: w.days_of_week ? [...w.days_of_week] : undefined }));` (deep copy 防源 mutate)
  - JSDoc: 复用 line 5-9 PeakWindow 注释模式（preset 默认 → 用户覆盖 platform.extra.peak_hours）
- 验收: 函数 exported，TS 编译过；deep copy 测试（mutate 返回值不污染 doc）

### ST2: PeakHoursSection 加导入按钮 + 确认 modal
- 文件: src/pages/platforms/formSections.tsx
- 改动:
  - props 加 `protocol: Protocol`
  - import 加 getDefaultPeakHours + useState (modalOpen + defaultWindows cache)
  - section `action` 槽（line 414-428 时区切换 div）旁加「导入默认配置」button:
    - onClick: `const def = await getDefaultPeakHours(protocol); if (def.length === 0) return; setDefaultCache(def); setModalOpen(true);`
    - disabled 当 `defaultCache === null`（首次未加载）→ 实际：hover 前 useEffect 预拉 default，cache 长度判 disabled
    - title tooltip: 无默认时 t("platform.peak_hours_no_default", "该平台无默认高峰配置")
  - 确认 modal: createPortal(document.body)，标题 `peak_hours_overwrite_confirm_title`，正文列将覆盖的窗口数 + 警告「当前配置将被丢弃」，确认/取消按钮；确认 → `setWindows(defaultCache.map(w => ({...w})))`；modal-window-center-rule 风格（fixed + transform centered）
- 验收: 按钮 disabled/enable 态正确；确认 → 全量替换；取消 → 不变；modal 居中

### ST3: caller 传 protocol + i18n key
- 文件:
  - src/pages/platforms/PlatformEditForm.tsx:203 `<PeakHoursSection ... protocol={protocol} />`
  - src/locales/{en-US,zh-Hans,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES}.json: 加 3 key
    - `platform.peak_hours_import_default`: "导入默认配置" / "Import Default"
    - `platform.peak_hours_no_default`: "该平台无默认高峰配置" / "No default peak config for this platform"
    - `platform.peak_hours_overwrite_confirm_title`: "覆盖高峰配置？" / "Overwrite peak config?"
    - `platform.peak_hours_overwrite_confirm_body`: "当前高峰配置将被默认值替换（{{count}} 个窗口），此操作不可撤销。" / "Current peak config will be replaced with default ({{count}} windows). Irreversible."
- 验收: yarn build 绿；8 语言 key 齐

## 验证
- yarn build（tsc + vite）绿
- 手动: 临时给 platform-presets.json 某 protocol 加 peak_hours → 按钮启用 → 导入 → 还原 JSON
- grep 调用点: PeakHoursSection caller 仅 PlatformEditForm.tsx:203，无遗漏

## 失败处理
- getDefaultPeakHours 返 undefined 类型错 → defaults.ts:47 类型已含 peak_hours，无新类型
- modal 居中失败 → 必 createPortal(document.body)（非 portal 内 fixed 受祖先 transform 退化）
- i18n 漏语言 → 跑 scripts/check-i18n.mjs 验

## 资源
- spec: .trellis/spec/guides/code-reuse-rules.md (deep copy 模式参考 getDefaultModels)
- memory: modal-window-center-rule
- CLAUDE.md peak_hours 段 + UI/i18n 段
