// PlatformEditForm — Platforms 编辑/新建表单（全屏视图，showForm=true 时渲染）。
// ponytail: 从 Platforms 主组件抽出的纯展示组件。所有 state 经 props 从 usePlatformsState 传入，
//   不持有自己的 state；表单分区（endpoints/models/budgets/breaker/group/expires）
//   均为 props 驱动的 JSX，无跨组件 setState。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { SmartPasteModal } from "../../components/platforms/SmartPasteModal";
import {
  SearchableProtocolSelect, MockConfigEditor,
  type ProtocolOption,
} from "../../domains/platforms";
import { getProtocolLabel, getProtocolColorMap, buildProtocolsFromPresets } from "../../domains/platforms/defaults";
import { useThemeMode } from "../../themes/useThemeMode";
import type { PlatformsState } from "./usePlatformsState";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
// 表单分区组件（9 个 section + FormSection / ApiKeyField / toDatetimeLocal）从此导入。
// ponytail: 抽到 formSections.tsx 以控制本文件行数；主组件仅消费 props 派发。
import {
  FormSection, ApiKeyField,
  DevinConfigSection, PassthroughConfigSection, EndpointsSection,
  BreakerSection, PeakSection, RateLimitsSection, GroupAssignSection,
  ExpirySection, QuotaSection,
} from "./formSections";
import { ModelsMatrixSection } from "./ModelsMatrixSection";
import { CcMitmAccessSection } from "./CcMitmAccessSection";
import { MultiKeyPreview } from "./MultiKeyPreview";
import { SectionTabs } from "../../components/shared";
import { makeRipple } from "../../components/shared";

export function PlatformEditForm({ s }: { s: PlatformsState }) {
  const { t, i18n } = useTranslation();
  const themeMode = useThemeMode();

  // 协议本地化 label 映射（key → JSON name）。fallback: PROTOCOL_LABELS 硬编码 → key。
  // docPromise 单次 RPC 缓存；切语言重拉。同 SearchableProtocolSelect:30-41 模式。
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  // ponytail: slice 化后 form 字段全部从 s.form 读，list/quota slice 本视图不直接消费。
  const {
    editing, showPaste, pasteInitialText, setShowPaste, setPasteInitialText,
    name, setName, protocol, codingPlan, handleProtocolChange,
    isMock, isPassthrough, keyOptional, apiKeyMissing,
    mockConfig, setMockConfig,
    apiKey, setApiKey, showKey, setShowKey,
    batchPreviewKeys, handleApiKeyChange, previewNames,
    quotaVariants, quotaVariantId, handleQuotaVariantChange, quotaSource, setQuotaSource,
    quotaCustomScript, setQuotaCustomScript,
    quotaRequires, setQuotaRequires,
    devinConfig, setDevinConfig,
    endpoints, setEndpoints,
    models, handleModelChange, handleModelSelect, activeDropdown, setActiveDropdown,
    availableModels, protocol: proto, fetching, fetchError, handleFetchModels, handleFillAll,
    manualBudgets, setManualBudgets,
    breakerDefaults, breakerFailureThreshold, setBreakerFailureThreshold,
    breakerOpenSecs, setBreakerOpenSecs, breakerHalfOpenMax, setBreakerHalfOpenMax,
    peak, setPeak, windowsTz, setWindowsTz,
    rateLimits, setRateLimits,
    disableDuringPeak, setDisableDuringPeak,
    mitmStats, setMitmStats,
    timeModels, setTimeModels,
    autoGroup, setAutoGroup, joinGroupIds, setJoinGroupIds, lockedGroupId,
    expiresAt, setExpiresAt, expiryEnabled, setExpiryEnabled,
    saveError,
    handleSave, resetForm, applyPaste,
  } = s.form;
  // groupDetails 在 list slice（list 态字段，经 listDeps 注入 form hook 但 owner 是 list）。
  const { groupDetails } = s.list;
  const { getPrimaryBaseUrl } = s;

  // ── 高级设置 tab（2026-10-08 改版）：内容全被条件隐藏的 tab 不进列表；只剩 1 个 → 不出条直接平铺 ──
  const advTabs = [
    ...(!isMock && !isPassthrough ? [{ id: "billing", label: t("platform.tabBilling", "计费") }] : []),
    ...(editing && !isPassthrough ? [{ id: "stability", label: t("platform.tabStability", "稳定性") }] : []),
    { id: "lifecycle", label: t("platform.tabLifecycle", "生命周期") },
  ];
  const [advTab, setAdvTab] = useState("billing");
  const activeAdvTab = advTabs.some(x => x.id === advTab) ? advTab : advTabs[0].id;

  // 协议本地化 label 映射（key → JSON name）。fallback: PROTOCOL_LABELS 硬编码（5 请求格式协议）→ key。
  // docPromise 单次 RPC 缓存；切语言重拉。同 SearchableProtocolSelect:30-41 模式。
  // ponytail: 仅拉取当前 protocol + editing.platform_type（编辑态）的 label，最小 RPC。
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const keys = Array.from(new Set([protocol, editing?.platform_type].filter(Boolean) as string[]));
      const entries = await Promise.all(
        keys.map(async k => [k, await getProtocolLabel(k as any, i18n.language)] as const)
      );
      if (!cancelled) setLabelMap(prev => ({ ...prev, ...Object.fromEntries(entries) }));
    })();
    return () => { cancelled = true; };
  }, [i18n.language, protocol, editing?.platform_type]);

  // 品牌色（async 派生自 registry platform.json 的 color）；首帧 fallback var(--accent)。
  const [colorMap, setColorMap] = useState<Partial<Record<string, string>>>({});
  useEffect(() => {
    let cancelled = false;
    getProtocolColorMap().then(m => { if (!cancelled) setColorMap(m); });
    return () => { cancelled = true; };
  }, []);
  // SmartPasteModal presets（ProtocolOption[]，hosts 内联派生）；首帧空数组（弹窗未开时不渲染）。
  const [presets, setPresets] = useState<ProtocolOption[]>([]);
  useEffect(() => {
    let cancelled = false;
    buildProtocolsFromPresets(i18n.language).then(list => { if (!cancelled) setPresets(list); });
    return () => { cancelled = true; };
  }, [i18n.language]);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* Edit page header */}
      <div className="section-header" style={{ gap: 10 }}>
        <Button variant="ghost" style={{ padding: "4px 8px", fontSize: 14 }} onClick={resetForm}>
          ← {t("action.back", "Back")}
        </Button>
        <div style={{ flex: 1 }}>
          <div className="section-title">
            {editing ? editing.name : t("platform.add")}
          </div>
          {editing && (
            <div className="section-desc">{labelMap[editing.platform_type] || editing.platform_type} · {getPrimaryBaseUrl(editing.platform_type, editing.endpoints ?? []) || editing.base_url}</div>
          )}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          {!editing && (
            <Button variant="outline" onClick={() => setShowPaste(true)}>
              {t("platform.paste.title", "智能识别")}
            </Button>
          )}
          <Button variant="outline" onClick={resetForm}>{t("action.cancel")}</Button>
          <Button className="ripple" onClick={(e) => { makeRipple(e); handleSave(); }}
            disabled={!name
              || (isPassthrough ? endpoints.length === 0 : (!isMock && !keyOptional && (endpoints.length === 0 || !apiKey)))}>
            {editing
              ? t("action.save")
              : batchPreviewKeys && batchPreviewKeys.length > 1
                ? t("platform.batch.createN", "批量创建（{{n}}）", { n: batchPreviewKeys.length })
                : t("action.create")}
          </Button>
        </div>
      </div>

      {showPaste && (
        <SmartPasteModal
          presets={presets}
          onApply={applyPaste}
          initialText={pasteInitialText}
          onClose={() => { setShowPaste(false); setPasteInitialText(undefined); }}
        />
      )}

      <div className="animate-fade-in" style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        {/* 基础信息：名称 + 协议 */}
        <FormSection title={t("platform.sectionBasic", "基础信息")}>
          <Input className="input" placeholder={t("platform.name")} value={name}
            onChange={(e) => setName(e.target.value)} />
          {editing ? (
            <div style={{
              display: "flex", alignItems: "center", gap: 8,
              padding: "10px 14px", borderRadius: "var(--radius-sm)",
              background: "var(--bg-glass)", border: "1px solid var(--border)",
              fontSize: 14,
            }}>
              <span style={{
                display: "inline-block", padding: "2px 8px", borderRadius: "var(--radius-sm)",
                background: `color-mix(in srgb, ${colorMap[protocol] || "var(--accent)"} 12%, transparent)`,
                color: colorMap[protocol] || "var(--accent)",
                fontSize: 11, fontWeight: 700,
              }}>
                {labelMap[protocol] || protocol}
              </span>
              <span style={{ color: "var(--text-tertiary)", fontSize: 12 }}>
                {t("platform.protocolLocked", "Protocol cannot be changed after creation")}
              </span>
            </div>
          ) : (
            <SearchableProtocolSelect
              value={protocol}
              codingPlan={codingPlan}
              onChange={handleProtocolChange}
            />
          )}
        </FormSection>

        {/* Mock 平台配置编辑器（仅 mock 平台显示，替代 endpoints / API Key / 模型） */}
        {isMock && (
          <FormSection title={t("platform.sectionSpecial", "特例配置")}>
            <MockConfigEditor config={mockConfig} onChange={setMockConfig} />
          </FormSection>
        )}


        {/* Devin 平台配置（可选 timeout/mode，仅 devin 协议显示；org_id 走上方 requires 表单） */}
        {protocol === "devin" && (
          <DevinConfigSection config={devinConfig} onChange={setDevinConfig} t={t} />
        )}

        {/* Claude Code 订阅（透传）配置：仅 base_url（host 根）+ 可空 api_key */}
        {isPassthrough && (
          <PassthroughConfigSection
            endpoints={endpoints} setEndpoints={setEndpoints}
            apiKey={apiKey} setApiKey={setApiKey}
            showKey={showKey} setShowKey={setShowKey}
            t={t}
          />
        )}

        {/* 订阅透传 MITM 接入形态开关 + Root CA 引导（cc-sub-mitm 票 12） */}
        {isPassthrough && (
          <CcMitmAccessSection
            enabled={mitmStats}
            onToggle={setMitmStats}
            groupName={
              editing
                ? groupDetails.find(g => g.platforms.some(gp => gp.platform.id === editing.id))?.group.name
                : undefined
            }
          />
        )}

        {/* Protocol Endpoints（mock / 透传平台隐藏，无可编辑上游） */}
        {!isMock && !isPassthrough && (
        <>
        <EndpointsSection endpoints={endpoints} setEndpoints={setEndpoints} protocol={protocol} t={t} />

        {/* Token（创建态多行 = 每行一个，批量创建；编辑态单行单平台） */}
        <FormSection title={t("platform.sectionAuth", "认证")}>
          <ApiKeyField
            value={apiKey} onChange={handleApiKeyChange} show={showKey} onToggleShow={() => setShowKey(!showKey)}
            editing={!!editing} multiline={!editing}
            placeholder={editing
              ? t("platform.tokenPlaceholderEdit", "Token")
              : t("platform.tokenPlaceholder", "Token（每行一个，多行将批量创建平台）")}
          />
        </FormSection>

        {/* 多 key 批量创建实时预览（创建态 + 非 keyOptional + 多 key 时触发，D1/D2/D3）。
            只读确认：name 自动生成 `{base}-{key尾4位}` 撞名追号，确认后复用 runBatchCreateFromPaste。 */}
        {batchPreviewKeys && batchPreviewKeys.length > 1 && !isMock && !isPassthrough && !keyOptional && (
          <MultiKeyPreview
            keys={batchPreviewKeys}
            previewNames={previewNames}
            protocol={protocol}
            baseUrl={getPrimaryBaseUrl(protocol, endpoints)}
            t={t}
          />
        )}

        {/* Models Matrix — 默认列 + 时段档列合并 card（PRD 07-09） */}
        <ModelsMatrixSection
          models={models} handleModelChange={handleModelChange} handleModelSelect={handleModelSelect}
          activeDropdown={activeDropdown} setActiveDropdown={setActiveDropdown}
          availableModels={availableModels} protocol={proto}
          fetchError={fetchError} fetching={fetching}
          onFillAll={handleFillAll} onFetchModels={handleFetchModels}
          apiKeyMissing={apiKeyMissing} endpointsCount={endpoints.length}
          rules={timeModels} setRules={setTimeModels} peak={peak}
          tzMode={windowsTz} setTzMode={setWindowsTz}
          t={t}
        />
        </>
        )}

        {/* ── 高级设置 tab（2026-10-08 改版，方向 A）：sticky 分段条 + 三个 panel ── */}
        {advTabs.length > 1 && (
          <SectionTabs tabs={advTabs} active={activeAdvTab} onChange={setAdvTab} />
        )}
        {/* 计费：配额查询 + 高峰倍率 + 时段档 */}
        {(!isMock && !isPassthrough) && (advTabs.length === 1 || activeAdvTab === "billing") && (
          <>
        {/* 配额查询合区（quota-ia 票 03）：模型矩阵正下方，Tab「自动脚本 / 手动预算」互斥；
            表单内切换只弹确认 + 记忆目标，清空在保存时由后端执行。mock / 透传无上游配额，整块不渲染。 */}
        {!isMock && !isPassthrough && (
          <QuotaSection
            quotaSource={quotaSource} onSourceChange={setQuotaSource}
            protocol={protocol}
            variants={quotaVariants}
            variantId={quotaVariantId}
            onVariantChange={handleQuotaVariantChange}
            customScript={quotaCustomScript}
            onCustomScriptChange={setQuotaCustomScript}
            requires={quotaRequires}
            onRequiresChange={(k, v) => setQuotaRequires(prev => ({ ...prev, [k]: v }))}
            locale={i18n.language}
            budgets={manualBudgets} setBudgets={setManualBudgets}
            editing={!!editing}
            t={t}
          />
        )}

        {/* Peak Hours 高峰/低峰倍率（仅编辑态可配；空数组 = 用 preset 默认 / 1.0） */}
        {editing && !isPassthrough && (
          <PeakSection
            windows={peak} setWindows={setPeak}
            tzMode={windowsTz} setTzMode={setWindowsTz}
            disableDuringPeak={disableDuringPeak} setDisableDuringPeak={setDisableDuringPeak}
            protocol={protocol}
            themeMode={themeMode}
            t={t}
          />
        )}

        {/* 时段档（时段模型切换）：从模型矩阵挪进计费 tab（2026-10-08 决策） */}
        <ModelsMatrixSection
          columns="time"
          titleOverride={t("platform.time_windows_section_title", "时段档")}
          models={models} handleModelChange={handleModelChange} handleModelSelect={handleModelSelect}
          activeDropdown={activeDropdown} setActiveDropdown={setActiveDropdown}
          availableModels={availableModels} protocol={proto}
          fetchError={fetchError} fetching={fetching}
          onFillAll={handleFillAll} onFetchModels={handleFetchModels}
          apiKeyMissing={apiKeyMissing} endpointsCount={endpoints.length}
          rules={timeModels} setRules={setTimeModels} peak={peak}
          tzMode={windowsTz} setTzMode={setWindowsTz}
          t={t}
        />
          </>
        )}
        {/* 稳定性：熔断 + 限频（2026-10-08 改版收进 tab） */}
        {editing && !isPassthrough && (advTabs.length === 1 || activeAdvTab === "stability") && (
          <>
        {/* Circuit Breaker 熔断覆盖（仅编辑态可配；空 = 继承全局默认） */}
        {editing && !isPassthrough && (
          <BreakerSection
            defaults={breakerDefaults}
            failure={breakerFailureThreshold} setFailure={setBreakerFailureThreshold}
            openSecs={breakerOpenSecs} setOpenSecs={setBreakerOpenSecs}
            halfOpenMax={breakerHalfOpenMax} setHalfOpenMax={setBreakerHalfOpenMax}
            t={t}
          />
        )}

        {/* 限频配置（rate-limit-aware 票 04：extra.rate_limits / extra.quota_windows） */}
        {editing && !isPassthrough && (
          <RateLimitsSection bundle={rateLimits} setBundle={setRateLimits} t={t} />
        )}

          </>
        )}
        {/* 生命周期：分组归属 + 过期时间（2026-10-08 改版收进 tab） */}
        {(advTabs.length === 1 || activeAdvTab === "lifecycle") && (
          <>
        {/* 分组归属 */}
        {!isPassthrough && (
          <GroupAssignSection
            editing={editing} lockedGroupId={lockedGroupId} groupDetails={groupDetails}
            autoGroup={autoGroup} setAutoGroup={setAutoGroup}
            joinGroupIds={joinGroupIds} setJoinGroupIds={setJoinGroupIds}
            t={t}
          />
        )}

        {/* 过期时间（可选） */}
        <ExpirySection
          expiresAt={expiresAt} setExpiresAt={setExpiresAt}
          expiryEnabled={expiryEnabled} setExpiryEnabled={setExpiryEnabled}
          themeMode={themeMode}
          t={t}
        />

          </>
        )}

        {saveError && (
          <div className="toast" style={{ fontSize: 12, wordBreak: "break-all" }}>
            {saveError}
          </div>
        )}
      </div>
    </div>
  );
}
