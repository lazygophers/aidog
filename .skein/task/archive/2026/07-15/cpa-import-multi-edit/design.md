# CPA 多 provider 导入改确认+多平台编辑页 — 详细设计

## 数据流
```
CpaImportModal onApplied(providers[])
  → providers.length === 1: applyCpaToForm (不变, 灌单条创建表单)
  → providers.length >= 2: setShowCpaMultiEdit(providers) (新: 打开手风琴编辑页)

CpaMultiEditModal (新组件, createPortal document.body)
  ├─ 顶部: provider 切换条 (N 个 chip, 点跳该项, 当前展开项高亮)
  ├─ 手风琴体: N 个 CpaProviderEditItem, 同时只展开 activeIdx 项
  │    └─ CpaProviderEditItem (子组件, 每项独立)
  │         ├─ usePlatformForm({...s}) 实例 (预填: applyCpaToForm(provider) 逻辑内联或复用)
  │         ├─ 展开时渲染 formSections (完整编辑表单, 复用 PlatformEditForm 的 section 结构)
  │         ├─ token 区: protocol.startsWith("cpa-") ? OAuth只读掩码 : api_key输入框
  │         └─ forwardRef 暴露 save(): 调本项 handleSave (create)
  └─ 底部: 「全部保存」按钮 → 遍历 N item ref 调 save(), 汇总报告

CpaProviderEditItem.save():
  → handleSave (复用 usePlatformForm 现有, create 路径)
  → 成功: 标记 done, 折叠
  → 失败: 返 error, 留页内标红

底部批量保存:
  for item in items: await item.save()
  全成功 → 关闭编辑页 + refreshPlatforms
  部分失败 → 留页, 失败项标红 + toast 汇总 "N 成功 M 失败"
```

## 关键取舍

### A. 每项独立 usePlatformForm 实例 (非单表单 state 数组)
- 原因: usePlatformForm 内聚表单 state + handlers(handleSave/applyPaste 等), 单实例管单平台。N 项 = N 独立表单 state, 最自然是 N 子组件各挂一实例
- 子组件 `CpaProviderEditItem` 接收 `{ provider, s }`, 内部 `const f = usePlatformForm({ ...s, skipResetOnMount })` 预填 provider
- 预填: 复用 applyCpaToForm 逻辑(现有 platformPasteApply.ts:357), 但 applyCpaToForm 操作外层单实例 state(setName/setApiKey 等) — 子组件需内联等价预填(直接调 f.setName 等) 或重构 applyCpaToForm 接收 form 实例

### B. token 区差异化靠 protocol 前缀
- MappedPlatform 无 oauth_type; cpa-* 协议(从 mapper.rs 产出)= OAuth provider, token=access_token(来自 auth_dir, 前端不该编辑)
- 非 cpa-* 协议(openai-compat/api-key 段)= 普通 api_key 可编辑
- 渲染分支: `protocol.startsWith("cpa-")` ? `<ReadOnlyMaskedToken value={apiKey}/>` : `<ApiKeyInput .../>`
- ReadOnlyMaskedToken: 掩码(首尾4 + ***) + 「来自 OAuth 凭据目录」提示, 不可编辑

### C. 底部批量保存 = 逐项 handleSave 串行
- 复用 usePlatformForm.handleSave(create 路径), 禁并发(N 并发 create 可能压后端 + refreshPlatforms 竞态)
- 串行 for-await, 每项成功标记 done, 失败收集
- 成功不回滚(已 create 的 platform 留库), 失败项留页内改后重试(只重存失败项)
- 不复用 runBatchCreateFromCpa(那是无编辑直接 create; 本场景需先编辑再 create, handleSave 已含完整 payload 构造)

### D. 取消防误丢
- 编辑页有未保存改动 → 关闭/取消弹确认(复用 navGuard 注册 或 createPortal confirm modal)
- 全部已保存(done) → 直接关无需确认

### E. 手风琴展开态
- activeIdx state(默认 0), 同时只展开 1 项
- 切换条点 provider 名 → setActiveIdx
- 已 done 项在切换条显 ✓ 标记
- 失败项在切换条显红点

## 改动文件
1. `src/components/platforms/CpaMultiEditModal.tsx` — 新建: 手风琴容器 + 顶部切换条 + 底部批量保存
2. `src/components/platforms/CpaProviderEditItem.tsx` — 新建: 单项编辑子组件(usePlatformForm 实例 + formSections + token 区分支 + forwardRef save)
3. `src/pages/platforms/PlatformEditForm.tsx` — onApplied 多条分支: runBatchCreateFromCpa → setShowCpaMultiEdit; 挂载 CpaMultiEditModal
4. `src/pages/platforms/usePlatformForm.ts` — applyCpaToForm 预填逻辑抽取可复用(或子组件内联等价); handleSave 暴露可被 forwardRef 调
5. `src/components/platforms/formSections.tsx` — token 区加 OAuth 只读掩码渲染分支(protocol.startsWith("cpa-"))
6. `src/locales/*.json`(8 个) — 新 key: cpaMultiEdit.title / saveAll / partialFail / cancelConfirm / oauthTokenHint 等

## 不改
- 后端 cpa_import.rs / cpa_import_parse / mapper.rs(纯前端改)
- services/api/platforms.ts(cpaImportApi 不变)
- 单 provider 路径(applyCpaToForm 灌单条表单不变)
- SmartPasteModal / MultiKeyPreview(独立路径)
- runBatchCreateFromCpa 保留(暂不删, 底部批量保存如复用其 create 循环则保留; 否则后续清理)

## 待 grill 澄清
1. usePlatformForm 多实例化是否可行(依赖外层 usePlatformsState 的 s.applyCpaToForm/handleSave 是单实例方法 — 多实例是否冲突?)
2. applyCpaToForm 预填逻辑复用方式(内联 vs 重构接收 form 实例)
3. 底部批量保存复用 handleSave vs runBatchCreateFromCpa 的 create 循环
4. token 区 OAuth 只读: 是否所有 cpa-* 协议都该只读? (Codex/Claude OAuth 的 access_token 用户可能想手动覆盖?)
