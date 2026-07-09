// PlatformEditForm — Platforms 编辑/新建表单（全屏视图，showForm=true 时渲染）。
// ponytail: 从 Platforms 主组件抽出的纯展示组件。所有 state 经 props 从 usePlatformsState 传入，
//   不持有自己的 state；表单分区（endpoints/models/budgets/breaker/group/expires/claude/middleware）
//   均为 props 驱动的 JSX，无跨组件 setState。
import { useTranslation } from "react-i18next";
import { SmartPasteModal } from "../../components/platforms/SmartPasteModal";
import { MiddlewareRulesPanel } from "../../components/settings/MiddlewareRules";
import {
  PROTOCOLS, PROTOCOL_COLORS,
  SearchableProtocolSelect, MockConfigEditor,
} from "../../domains/platforms";
import { useThemeMode } from "../../themes/useThemeMode";
import type { PlatformsState } from "./usePlatformsState";
// 表单分区组件（9 个 section + FormSection / ApiKeyField / toDatetimeLocal）从此导入。
// ponytail: 抽到 formSections.tsx 以控制本文件行数；主组件仅消费 props 派发。
import {
  FormSection, ApiKeyField,
  NewApiBalanceConfigSection, PassthroughConfigSection, EndpointsSection,
  ManualBudgetsSection, BreakerSection, PeakHoursSection, GroupAssignSection,
  ExpirySection, ClaudeConfigSection,
} from "./formSections";
import { ModelsMatrixSection } from "./ModelsMatrixSection";
import { MultiKeyPreview } from "./MultiKeyPreview";

export function PlatformEditForm({ s }: { s: PlatformsState }) {
  const { t } = useTranslation();
  const themeMode = useThemeMode();
  const {
    editing, showPaste, pasteInitialText, setShowPaste, setPasteInitialText,
    name, setName, protocol, codingPlan, handleProtocolChange,
    isMock, isPassthrough, keyOptional, apiKeyMissing,
    mockConfig, setMockConfig,
    apiKey, setApiKey, showKey, setShowKey,
    batchPreviewKeys, handleApiKeyChange, confirmBatchCreate, cancelBatchPreview, previewNames,
    newApiConfig, setNewApiConfig,
    endpoints, setEndpoints,
    models, handleModelChange, handleModelSelect, activeDropdown, setActiveDropdown,
    availableModels, protocol: proto, fetching, fetchError, handleFetchModels, handleFillAll,
    manualBudgets, setManualBudgets,
    breakerDefaults, breakerFailureThreshold, setBreakerFailureThreshold,
    breakerOpenSecs, setBreakerOpenSecs, breakerHalfOpenMax, setBreakerHalfOpenMax,
    peakHours, setPeakHours, peakHoursTz, setPeakHoursTz,
    disableDuringPeak, setDisableDuringPeak,
    timeModels, setTimeModels,
    autoGroup, setAutoGroup, joinGroupIds, setJoinGroupIds, lockedGroupId,
    groupDetails, levelPriority, setLevelPriority,
    expiresAt, setExpiresAt, expiryEnabled, setExpiryEnabled,
    uniqueGroupInfo,
    showClaudeConfig, setShowClaudeConfig, claudeConfigJson, setClaudeConfigJson,
    globalClaudeConfig,
    saveError,
    handleSave, resetForm, applyPaste, getPrimaryBaseUrl,
  } = s;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* Edit page header */}
      <div className="section-header" style={{ gap: 10 }}>
        <button className="btn btn-ghost" style={{ padding: "4px 8px", fontSize: 14 }} onClick={resetForm}>
          ← {t("action.back", "Back")}
        </button>
        <div style={{ flex: 1 }}>
          <div className="section-title">
            {editing ? editing.name : t("platform.add")}
          </div>
          {editing && (
            <div className="section-desc">{editing.platform_type.toUpperCase()} · {getPrimaryBaseUrl(editing.platform_type, editing.endpoints ?? []) || editing.base_url}</div>
          )}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          {!editing && (
            <button className="btn" onClick={() => setShowPaste(true)}>
              {t("platform.paste.title", "智能识别")}
            </button>
          )}
          <button className="btn" onClick={resetForm}>{t("action.cancel")}</button>
          <button className="btn btn-primary" onClick={handleSave}
            disabled={!name
              || (!!batchPreviewKeys && batchPreviewKeys.length > 1)
              || (isPassthrough ? endpoints.length === 0 : (!isMock && !keyOptional && (endpoints.length === 0 || !apiKey)))}>
            {editing ? t("action.save") : t("action.create")}
          </button>
        </div>
      </div>

      {showPaste && (
        <SmartPasteModal
          presets={PROTOCOLS}
          onApply={applyPaste}
          initialText={pasteInitialText}
          onClose={() => { setShowPaste(false); setPasteInitialText(undefined); }}
        />
      )}

      <div className="animate-fade-in" style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        {/* 基础信息：名称 + 协议 */}
        <FormSection title={t("platform.sectionBasic", "基础信息")}>
          <input className="input" placeholder={t("platform.name")} value={name}
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
                background: `${PROTOCOL_COLORS[protocol] || "var(--accent)"}20`,
                color: PROTOCOL_COLORS[protocol] || "var(--accent)",
                fontSize: 11, fontWeight: 700,
              }}>
                {protocol.toUpperCase()}
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

        {/* New API 余额查询配置（仅 newapi 平台显示） */}
        {protocol === "newapi" && (
          <NewApiBalanceConfigSection config={newApiConfig} onChange={setNewApiConfig} t={t} />
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

        {/* Protocol Endpoints（mock / 透传平台隐藏，无可编辑上游） */}
        {!isMock && !isPassthrough && (
        <>
        <EndpointsSection endpoints={endpoints} setEndpoints={setEndpoints} t={t} />

        {/* API Key */}
        <FormSection title={t("platform.sectionAuth", "认证")}>
          <ApiKeyField value={apiKey} onChange={handleApiKeyChange} show={showKey} onToggleShow={() => setShowKey(!showKey)} editing={!!editing} />
        </FormSection>

        {/* 多 key 批量创建实时预览（创建态 + 非 keyOptional + 多 key 时触发，D1/D2/D3）。
            只读确认：name 自动生成 `{base}-{key尾4位}` 撞名追号，确认后复用 runBatchCreateFromPaste。 */}
        {batchPreviewKeys && batchPreviewKeys.length > 1 && !isMock && !isPassthrough && !keyOptional && (
          <MultiKeyPreview
            keys={batchPreviewKeys}
            previewNames={previewNames}
            protocol={protocol}
            baseUrl={getPrimaryBaseUrl(protocol, endpoints)}
            onConfirm={confirmBatchCreate}
            onCancel={cancelBatchPreview}
            t={t}
          />
        )}

        {/* Models Matrix — 默认列 + 时段档列合并 card（PRD 07-09） */}
        <ModelsMatrixSection
          models={models} handleModelChange={handleModelChange} handleModelSelect={handleModelSelect}
          activeDropdown={activeDropdown} setActiveDropdown={setActiveDropdown}
          availableModels={availableModels} protocol={proto} codingPlan={codingPlan}
          fetchError={fetchError} fetching={fetching}
          onFillAll={handleFillAll} onFetchModels={handleFetchModels}
          apiKeyMissing={apiKeyMissing} endpointsCount={endpoints.length}
          rules={timeModels} setRules={setTimeModels} peakHours={peakHours}
          t={t}
        />
        </>
        )}

        {/* Manual Budgets */}
        {!isPassthrough && (
          <ManualBudgetsSection budgets={manualBudgets} setBudgets={setManualBudgets} t={t} />
        )}

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

        {/* Peak Hours 高峰/低峰倍率（仅编辑态可配；空数组 = 用 preset 默认 / 1.0） */}
        {editing && !isPassthrough && (
          <PeakHoursSection
            windows={peakHours} setWindows={setPeakHours}
            tzMode={peakHoursTz} setTzMode={setPeakHoursTz}
            disableDuringPeak={disableDuringPeak} setDisableDuringPeak={setDisableDuringPeak}
            protocol={protocol}
            t={t}
          />
        )}

        {/* 分组归属 */}
        {!isPassthrough && (
          <GroupAssignSection
            editing={editing} lockedGroupId={lockedGroupId} groupDetails={groupDetails}
            autoGroup={autoGroup} setAutoGroup={setAutoGroup}
            joinGroupIds={joinGroupIds} setJoinGroupIds={setJoinGroupIds}
            uniqueGroupInfo={uniqueGroupInfo}
            levelPriority={levelPriority} setLevelPriority={setLevelPriority}
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

        {/* Claude Code Config */}
        {editing && (
          <ClaudeConfigSection
            show={showClaudeConfig} setShow={setShowClaudeConfig}
            json={claudeConfigJson} setJson={setClaudeConfigJson}
            globalConfig={globalClaudeConfig}
            t={t}
          />
        )}

        {/* Middleware rules (platform scope) — 需已有 platform_id */}
        {editing && (
          <FormSection
            title={t("middleware.platformRules", "平台中间件规则")}
            desc={t("middleware.platformRulesHint", "仅本平台生效，就近覆盖分组 / 全局同类型规则")}
          >
            <MiddlewareRulesPanel scope="platform" scopeRef={String(editing.id)} embedded />
          </FormSection>
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
