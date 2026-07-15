# CPA 导入填表单模式 — 设计

## 改动面
4 文件: CpaImportModal.tsx / PlatformEditForm.tsx / platformPasteApply.ts(加 2 函数) / src/locales/*.json(8)。services/api cpaImportApi.apply 标注废弃。

## 范本
SmartPaste fullShare 填表单(platformPasteApply.ts:81-114): setName/setProtocol/setApiKey/setModels/setEndpoints/setExtra + setEditing(null) + setShowForm(true)。
runBatchCreateFromPaste 多 key 批量(platformPasteApply.ts:283-355): 逐个 platformApi.create + 进度 toast + 失败收集 + group 用表单(line 300-301)。

## A. applyCpaToForm(provider, ctx) — 单条填表单
新函数(加 platformPasteApply.ts 或新 platformCpaApply.ts):
```ts
export async function applyCpaToForm(p: MappedPlatform, ctx: PlatformPasteCtx): Promise<void> {
  const { setName, setApiKey, setAvailableModels, setExtra, setMockConfig,
          setNewApiConfig, setBreakerFailureThreshold, setBreakerOpenSecs, setBreakerHalfOpenMax,
          setEditing, setShowCpaImport, setShowForm, handleProtocolChange } = ctx;
  setName(p.name);
  await handleProtocolChange(p.protocol, false);  // 填默认 endpoints + client_type
  setApiKey(p.api_key);
  setAvailableModels(p.models ?? []);
  // 覆盖 endpoints base_url 为 provider 的 base_url(单 endpoint)
  // (handleProtocolChange 已填默认 endpoints, 这里覆盖 base_url; 若 provider base_url 非空)
  if (p.base_url) {
    // setEndpoints([{ protocol: p.protocol, base_url: p.base_url, client_type: await defaultClientForProtocol(p.protocol) }])
    // 或: 保留默认 endpoints 结构, 覆盖首条 base_url
  }
  const ex = p.extra ?? "";
  setExtra(ex);
  setMockConfig(parseMockConfig(ex));
  setNewApiConfig(parseNewApiConfig(ex));
  const brk = parsePlatformBreaker(ex);
  setBreakerFailureThreshold(brk.failure_threshold > 0 ? String(brk.failure_threshold) : "");
  setBreakerOpenSecs(brk.open_secs > 0 ? String(brk.open_secs) : "");
  setBreakerHalfOpenMax(brk.half_open_max > 0 ? String(brk.half_open_max) : "");
  setEditing(null);       // 新建态
  setShowCpaImport(false);
  setShowForm(true);      // 开表单
}
```

注意: disabled 字段(CPA provider 可 disabled=true)— 填表单模式用户手动设 status, 不自动 disable(表单无 status 字段, handleSave 不接 status)。design 决定: 忽略 disabled(填表单让用户决定)。

## B. runBatchCreateFromCpa(providers, ctx) — 多条批量
新函数(同文件):
```ts
export async function runBatchCreateFromCpa(providers: MappedPlatform[], ctx: PlatformPasteCtx): Promise<void> {
  const { t, lockedGroupId, joinGroupIds, autoGroup, expiresAt,
          setPlatforms, platformsEpochRef, quota, handleGroupsChanged, groupsReloadRef,
          resetForm, setToast, setShowCpaImport } = ctx;
  const joinIds = lockedGroupId != null ? [lockedGroupId] : joinGroupIds;
  const auto = lockedGroupId != null ? false : autoGroup;
  // 进度 toast + 逐个 create(各 provider 独立 protocol/base_url/api_key/models)
  // 范本 runBatchCreateFromPaste:283-355, 但每 provider 各自 protocol/base_url(非共享)
  let okCount = 0;
  const failures = [];
  setToast({ text: t("platform.cpaImport.batchProgress", "批量创建中… {{done}}/{{total}}", { done: 0, total: providers.length }), ok: true });
  for (let i = 0; i < providers.length; i++) {
    const p = providers[i];
    try {
      const ep = p.base_url ? [{ protocol: p.protocol, base_url: p.base_url, client_type: await defaultClientForProtocol(p.protocol) }] : undefined;
      const saved = await platformApi.create({
        name: p.name, platform_type: p.protocol, base_url: p.base_url || "", api_key: p.api_key,
        endpoints: ep, available_models: p.models?.length ? p.models : undefined,
        extra: p.extra || undefined,
        auto_group: auto, join_group_ids: joinIds, expires_at: expiresAt,
      });
      // append + quota + 进度 toast(同 runBatchCreateFromPaste)
      okCount++;
    } catch (e: any) { failures.push({ name: p.name, err: String(e) }); }
  }
  handleGroupsChanged(); groupsReloadRef.current?.();
  window.dispatchEvent(new Event("aidog-groups-changed"));
  resetForm(); setShowCpaImport(false);
  // 末尾汇总 toast(成功 X / 失败 Y + 失败名)
}
```

## C. CpaImportModal onApplied 改签名
CpaImportModal.tsx:
- props onApplied: `(providers: MappedPlatform[]) => void | Promise<void>`(替代 created/failed)
- handleApply: 收集选中 providers → `onApplied(providers)`(移除 cpaImportApi.apply 调用)
- 按钮文案: 选中 1 → t("platform.cpaImport.applyOne", "填入表单"); 选中 N → t("platform.cpaImport.applyBatch", "批量创建 {{n}} 个")
- 移除 group badge(填表单模式表单有 GroupAssignSection)

## D. PlatformEditForm onApplied 接入
PlatformEditForm.tsx:147-156 改:
```tsx
onApplied={async (providers) => {
  const ctx = buildPasteCtx();  // 复用现有 ctx 构建
  if (providers.length === 1) {
    await applyCpaToForm(providers[0], ctx);  // 填表单, modal 关 + 表单开
  } else {
    await runBatchCreateFromCpa(providers, ctx);  // 批量创建 + toast
  }
}}
```

## E. i18n(8 locale)
新 key: `platform.cpaImport.applyOne` / `applyBatch` / `batchProgress` / `batchAllOk` / `batchFailSummary`
- zh-Hans: "填入表单" / "批量创建 {{n}} 个" / "批量创建中… {{done}}/{{total}}" / "批量创建完成：成功 {{n}} 个" / "成功 {{ok}} / 失败 {{fail}} · {{detail}}"

## 废弃
- services/api cpaImportApi.apply: 标 @deprecated 注释(后端 cpa_import_apply 命令保留, 不再调用)
- CpaImportModal 移除 cpaImportApi import(若仅 apply 用)

## 不改
- cpa_import_parse / MappedPlatform / cpa_import.rs 后端命令
- SmartPaste applyPaste / runBatchCreateFromPaste(仅模式参考)

## F. disabled 处理(批量路径补)
CPA provider 可 disabled=true(openai-compatibility 段)。填表单模式: 忽略(用户手动设 status, handleSave 不接 status)。批量 runBatchCreateFromCpa: 保留语义 — disabled provider create 后 post-update status=disabled(同原 cpa_import.rs:115-128):
```ts
const saved = await platformApi.create({...});
if (p.disabled) {
  await platformApi.update(saved.id, { status: "disabled" });  // grep 确认 update 签名
}
```
单条填表单: 不 auto-disable(用户在表单改, 但表单无 status 字段 — disabled 语义在填表单模式丢失, 用户需创建后手 disable。接受, design 决定)。

## G. base_url 空(OAuth provider)处理
OAuth provider(CpaGrok/Anthropic/Codex/Kimi)p.base_url="" → 走 preset 协议默认 base_url。批量 create base_url="" + endpoints=undefined → 后端 router 用 platform.platform_type 的 preset 默认 endpoints。填表单单条: 用户可改 base_url。批量: 信任 preset(同 runBatchCreateFromPaste 无 base_url 校验的轻量模式)。
