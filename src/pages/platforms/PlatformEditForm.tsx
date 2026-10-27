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
import { buildProtocolsFromPresets } from "../../domains/platforms/defaults";
import { useProtocolLabelMap } from "../../domains/platforms/useProtocolMeta";
import { useThemeMode } from "../../themes/useThemeMode";
import type { PlatformsState } from "./usePlatformsState";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
// 表单分区组件（9 个 section + FormSection / ApiKeyField / toDatetimeLocal）从此导入。
// ponytail: 抽到 formSections.tsx 以控制本文件行数；主组件仅消费 props 派发。
import {
  FormSection, ApiKeyField,
  DevinConfigSection, PassthroughConfigSection, PassThroughSection, EndpointsSection,
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

  // ponytail: slice 化后 form 字段全部从 s.form 读，list/quota slice 本视图不直接消费。
  const {
    editing, showPaste, pasteInitialText, setShowPaste, setPasteInitialText,
    name, setName, protocol, codingPlan, handleProtocolChange,
    isMock, isPassthrough, isPassThrough, keyOptional, apiKeyMissing,
    passAuth, setPassAuth,
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
    timemodels,
    rateLimits, setRateLimits,
    mitmStats, setMitmStats,
    autoGroup, setAutoGroup, joinGroupIds, setJoinGroupIds, lockedGroupId,
    expiresAt, setExpiresAt, expiryEnabled, setExpiryEnabled,
    saveError,
    handleSave, resetForm, applyPaste,
  } = s.form;
  // groupDetails 在 list slice（list 态字段，经 listDeps 注入 form hook 但 owner 是 list）。
  const { groupDetails } = s.list;
  const { getPrimaryBaseUrl } = s;

  // ── 高级设置 tab（2026-10-20 拆 7 tab）：内容全被条件隐藏的 tab 不进列表；只剩 1 个 → 不出条直接平铺 ──
  const advTabs = [
    ...(!isMock && !isPassthrough && !isPassThrough ? [{ id: "quota", label: t("platform.tabQuota", "配额查询") }] : []),
    ...(!isMock && !isPassthrough && !isPassThrough ? [{ id: "timemodels", label: t("platform.tabTimemodels", "时段档") }] : []),
    ...(!isMock && editing && !isPassthrough && !isPassThrough ? [{ id: "peak", label: t("platform.tabPeak", "高峰时段倍率") }] : []),
    ...(editing && !isPassthrough && !isPassThrough ? [{ id: "ratelimits", label: t("platform.tabRateLimits", "限频") }] : []),
    ...(editing && !isPassthrough && !isPassThrough ? [{ id: "breaker", label: t("platform.tabBreaker", "熔断") }] : []),
    { id: "expiry", label: t("platform.tabExpiry", "过期时间") },
    ...(!isPassthrough ? [{ id: "group", label: t("platform.tabGroupAssign", "分组归属") }] : []),
  ];
  const [advTab, setAdvTab] = useState("quota");
  const activeAdvTab = advTabs.some(x => x.id === advTab) ? advTab : advTabs[0].id;

  // 协议本地化 label 映射（共享 hook 全表）。fallback: PROTOCOL_LABELS 硬编码 → key；
  // 切语言重拉。全表覆盖原「仅 2 key」最小拉取（docPromise 单例缓存，无额外 RPC）。
  const labelMap = useProtocolLabelMap(i18n.language);
  // 品牌色（async 派生自 registry platform.json 的 color）；首帧 fallback var(--accent)。
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
            <div className="section-desc">
              {/* unknown = DB platform_type 落了枚举外的值（票 06）：显式「未知协议」，提示重选平台类型 */}
              {editing.platform_type === "unknown" ? t("platform.unknownProtocol", "未知协议") : labelMap[editing.platform_type] || editing.platform_type} · {getPrimaryBaseUrl(editing.platform_type, editing.endpoints ?? []) || editing.base_url}
            </div>
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
            disabled={!name || protocol === "unknown"
              // passthrough（自定义透传）：base_url 与 api_key 都必填（共识稿 §2）
              || (isPassThrough ? (endpoints.length === 0 || !apiKey)
              : isPassthrough ? endpoints.length === 0
              : (!isMock && !keyOptional && (endpoints.length === 0 || !apiKey)))}>
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
          {/* 协议可改（2026-10-27 用户拍板）：编辑态与创建态同款选择器；unknown 修复/正常切换共用 handleProtocolChange */}
          <SearchableProtocolSelect
            value={protocol}
            codingPlan={codingPlan}
            onChange={handleProtocolChange}
          />
          {protocol === "unknown" && (
            <p role="alert">{t("platform.unknownProtocolSave", "请选择有效的协议类型后再保存")}</p>
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

        {/* 自定义透传（passthrough）配置：base_url + apikey + 认证 header 预设（共识稿 §2） */}
        {isPassThrough && (
          <PassThroughSection
            endpoints={endpoints} setEndpoints={setEndpoints}
            apiKey={apiKey} setApiKey={setApiKey}
            showKey={showKey} setShowKey={setShowKey}
            auth={passAuth} setAuth={setPassAuth}
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
        {!isMock && !isPassthrough && !isPassThrough && (
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
        {batchPreviewKeys && batchPreviewKeys.length > 1 && !isMock && !isPassthrough && !isPassThrough && !keyOptional && (
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
          rules={timemodels.timeModels} setRules={timemodels.setTimeModels} peak={timemodels.peak}
          tzMode={timemodels.windowsTz} setTzMode={timemodels.setWindowsTz}
          t={t}
        />
        </>
        )}

        {/* ── 高级设置（2026-10-08）：顶部 sticky 分段条 + panels；左菜单样式只用于时段档 tab 内部 ── */}
        {advTabs.length > 1 && (
          <SectionTabs tabs={advTabs} active={activeAdvTab} onChange={setAdvTab} />
        )}
        {/* 配额查询（quota-ia 票 03；2026-10-20 从计费 tab 拆出独立 tab）：Tab「自动脚本 / 手动预算」互斥；
            表单内切换只弹确认 + 记忆目标，清空在保存时由后端执行。mock / 透传无上游配额，整块不渲染。 */}
        {!isMock && !isPassthrough && !isPassThrough && (advTabs.length === 1 || activeAdvTab === "quota") && (
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

        {/* 高峰时段倍率（2026-10-20 从计费 tab 拆出；仅编辑态可配；空数组 = 用 preset 默认 / 1.0） */}
        {editing && !isPassthrough && !isPassThrough && (advTabs.length === 1 || activeAdvTab === "peak") && (
          <PeakSection
            windows={timemodels.peak} setWindows={timemodels.setPeak}
            tzMode={timemodels.windowsTz} setTzMode={timemodels.setWindowsTz}
            disableDuringPeak={timemodels.disableDuringPeak} setDisableDuringPeak={timemodels.setDisableDuringPeak}
            protocol={protocol}
            themeMode={themeMode}
            t={t}
          />
        )}

        {/* 时段档（时段模型切换）：独立 tab，2026-10-08 二轮裁决（原塞计费 tab） */}
        {(!isMock && !isPassthrough && !isPassThrough) && (advTabs.length === 1 || activeAdvTab === "timemodels") && (
          <>
        <ModelsMatrixSection
          columns="time"
          titleOverride={t("platform.time_windows_section_title", "时段档")}
          models={models} handleModelChange={handleModelChange} handleModelSelect={handleModelSelect}
          activeDropdown={activeDropdown} setActiveDropdown={setActiveDropdown}
          availableModels={availableModels} protocol={proto}
          fetchError={fetchError} fetching={fetching}
          onFillAll={handleFillAll} onFetchModels={handleFetchModels}
          apiKeyMissing={apiKeyMissing} endpointsCount={endpoints.length}
          rules={timemodels.timeModels} setRules={timemodels.setTimeModels} peak={timemodels.peak}
          tzMode={timemodels.windowsTz} setTzMode={timemodels.setWindowsTz}
          t={t}
        />
          </>
        )}
        {/* 限频（2026-10-20 从稳定性 tab 拆出；rate-limit-aware 票 04：extra.rate_limits / extra.quota_windows） */}
        {editing && !isPassthrough && !isPassThrough && (advTabs.length === 1 || activeAdvTab === "ratelimits") && (
          <RateLimitsSection bundle={rateLimits} setBundle={setRateLimits} t={t} />
        )}

        {/* 熔断（2026-10-20 从稳定性 tab 拆出）：Circuit Breaker 覆盖（仅编辑态可配；空 = 继承全局默认） */}
        {editing && !isPassthrough && !isPassThrough && (advTabs.length === 1 || activeAdvTab === "breaker") && (
          <BreakerSection
            defaults={breakerDefaults}
            failure={breakerFailureThreshold} setFailure={setBreakerFailureThreshold}
            openSecs={breakerOpenSecs} setOpenSecs={setBreakerOpenSecs}
            halfOpenMax={breakerHalfOpenMax} setHalfOpenMax={setBreakerHalfOpenMax}
            t={t}
          />
        )}

        {/* 过期时间（2026-10-20 从生命周期 tab 拆出；可选） */}
        {(advTabs.length === 1 || activeAdvTab === "expiry") && (
          <ExpirySection
            expiresAt={expiresAt} setExpiresAt={setExpiresAt}
            expiryEnabled={expiryEnabled} setExpiryEnabled={setExpiryEnabled}
            themeMode={themeMode}
            t={t}
          />
        )}

        {/* 分组归属（2026-10-20 从生命周期 tab 拆出；透传平台隐藏） */}
        {!isPassthrough && (advTabs.length === 1 || activeAdvTab === "group") && (
          <GroupAssignSection
            editing={editing} lockedGroupId={lockedGroupId} groupDetails={groupDetails}
            autoGroup={autoGroup} setAutoGroup={setAutoGroup}
            joinGroupIds={joinGroupIds} setJoinGroupIds={setJoinGroupIds}
            t={t}
          />
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
