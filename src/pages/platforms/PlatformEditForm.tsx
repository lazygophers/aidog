// PlatformEditForm — Platforms 编辑/新建表单（全屏视图，showForm=true 时渲染）。
// ponytail: 从 Platforms 主组件抽出的纯展示组件。所有 state 经 props 从 usePlatformsState 传入，
//   不持有自己的 state；表单分区（endpoints/models/budgets/breaker/group/expires/claude/middleware）
//   均为 props 驱动的 JSX，无跨组件 setState。
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { SmartPasteModal } from "../../components/platforms/SmartPasteModal";
import { CpaImportModal } from "../../components/platforms/CpaImportModal";
import { MiddlewareRulesPanel } from "../../components/settings/MiddlewareRules";
import { cliProxyApi, type CliProxyProvider } from "../../services/api";
import {
  SearchableProtocolSelect, MockConfigEditor,
  type ProtocolOption,
} from "../../domains/platforms";
import { getProtocolLabel, getProtocolColorMap, buildProtocolsFromPresets } from "../../domains/platforms/defaults";
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
  const { t, i18n } = useTranslation();
  const themeMode = useThemeMode();

  // 协议本地化 label 映射（key → JSON name）。fallback: PROTOCOL_LABELS 硬编码 → key。
  // docPromise 单次 RPC 缓存；切语言重拉。同 SearchableProtocolSelect:30-41 模式。
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
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
    handleSave, resetForm, applyPaste, getPrimaryBaseUrl, createCliProxyPlatform,
  } = s;

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

  // 品牌色（async 派生自 platform-presets.json）；首帧 fallback var(--accent)。
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
  // CpaImportModal 开关（仅新建态入口展示，同 SmartPaste 模式；apply 后由父级 refreshPlatforms 刷新列表）。
  const [showCpaImport, setShowCpaImport] = useState(false);
  // 「从 cli-proxy 添加」picker 开关（cpa-standalone-module s6）：新建态入口，
  // 选定 provider 后调 createCliProxyPlatform → 后端建 cli-proxy 平台 → 刷新列表 + 关表单。
  const [showCliProxyPicker, setShowCliProxyPicker] = useState(false);
  const [cliProxyProviders, setCliProxyProviders] = useState<CliProxyProvider[]>([]);
  // 编辑 cli-proxy 平台时反查 provider（models/base_url/api_key 只读自 provider）。
  const [cliProxyProvider, setCliProxyProvider] = useState<CliProxyProvider | null>(null);
  const isCliProxyEditing = !!editing && editing.platform_type === "cli-proxy";

  // picker 打开时拉 provider 列表（单次 RPC，关窗不重拉；cpa-standalone-module s6）。
  useEffect(() => {
    if (!showCliProxyPicker) return;
    let cancelled = false;
    cliProxyApi.list()
      .then(list => { if (!cancelled) setCliProxyProviders(list); })
      .catch(() => { if (!cancelled) setCliProxyProviders([]); });
    return () => { cancelled = true; };
  }, [showCliProxyPicker]);

  // 编辑 cli-proxy 平台时按 extra.cli_proxy_provider_id 拉 provider（只读展示 models/base_url/api_key 来源）。
  useEffect(() => {
    if (!isCliProxyEditing || !editing) { setCliProxyProvider(null); return; }
    let cancelled = false;
    try {
      const extra = editing.extra ? JSON.parse(editing.extra) : {};
      const pid = extra?.cli_proxy_provider_id;
      if (typeof pid !== "number") { setCliProxyProvider(null); return; }
      cliProxyApi.get(pid)
        .then(p => { if (!cancelled) setCliProxyProvider(p ?? null); })
        .catch(() => { if (!cancelled) setCliProxyProvider(null); });
    } catch { setCliProxyProvider(null); }
    return () => { cancelled = true; };
  }, [isCliProxyEditing, editing]);

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
            <div className="section-desc">{labelMap[editing.platform_type] || editing.platform_type} · {getPrimaryBaseUrl(editing.platform_type, editing.endpoints ?? []) || editing.base_url}</div>
          )}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          {!editing && (
            <>
              <button className="btn" onClick={() => setShowPaste(true)}>
                {t("platform.paste.title", "智能识别")}
              </button>
              <button className="btn" onClick={() => setShowCpaImport(true)}>
                {t("platform.cpaImport.entry", "导入 CPA 配置")}
              </button>
              <button className="btn" onClick={() => setShowCliProxyPicker(true)}>
                {t("platform.cliProxy.addFromProvider", "从 cli-proxy 添加")}
              </button>
            </>
          )}
          <button className="btn" onClick={resetForm}>{t("action.cancel")}</button>
          <button className="btn btn-primary" onClick={handleSave}
            disabled={!name
              || (!!batchPreviewKeys && batchPreviewKeys.length > 1)
              || (isCliProxyEditing
                  ? false
                  : (isPassthrough ? endpoints.length === 0 : (!isMock && !keyOptional && (endpoints.length === 0 || !apiKey))))}>
            {editing ? t("action.save") : t("action.create")}
          </button>
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

      {showCpaImport && (
        <CpaImportModal
          open={showCpaImport}
          onClose={() => setShowCpaImport(false)}
          onApplied={async (providers) => {
            // 填表单模式：单条 → 灌入创建表单（用户改配置 + 设 group + 保存）；
            // 多条 → 前端批量创建（各独立 platform，用表单 group）。
            setShowCpaImport(false);
            if (providers.length === 1) {
              await s.applyCpaToForm(providers[0]);
            } else {
              await s.runBatchCreateFromCpa(providers);
            }
          }}
        />
      )}

      {showCliProxyPicker && createPortal(
        <div onClick={() => setShowCliProxyPicker(false)} style={{
          position: "fixed", inset: 0, background: "rgba(0,0,0,0.45)",
          display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000,
        }}>
          <div onClick={e => e.stopPropagation()} style={{
            background: "var(--bg-elevated)", borderRadius: "var(--radius-md)",
            padding: 20, minWidth: 420, maxWidth: 560, maxHeight: "70vh",
            overflowY: "auto", border: "1px solid var(--border)",
            boxShadow: "var(--shadow-lg, 0 8px 32px rgba(0,0,0,0.3))",
          }}>
            <div style={{ fontSize: 16, fontWeight: 600, marginBottom: 12 }}>
              {t("platform.cliProxy.pickerTitle", "选择 cli-proxy provider")}
            </div>
            <div style={{ fontSize: 12, color: "var(--text-tertiary)", marginBottom: 12 }}>
              {t("platform.cliProxy.pickerHint", "选定后将立即创建 cli-proxy 平台并加入当前分组")}
            </div>
            {cliProxyProviders.length === 0 ? (
              <div style={{ fontSize: 13, color: "var(--text-tertiary)", padding: "20px 0", textAlign: "center" }}>
                {t("platform.cliProxy.pickerEmpty", "暂无 cli-proxy provider，请先在 cli-proxy 页导入")}
              </div>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {cliProxyProviders.map(p => (
                  <button key={p.id} className="btn btn-ghost" style={{
                    justifyContent: "flex-start", textAlign: "left", padding: "10px 12px",
                    border: "1px solid var(--border)",
                  }} onClick={async () => {
                    setShowCliProxyPicker(false);
                    await createCliProxyPlatform(p);
                  }}>
                    <div style={{ fontWeight: 600, fontSize: 13 }}>{p.name}</div>
                    <div style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                      {p.wire_protocol} · {p.base_url} · {p.models.length} models
                    </div>
                  </button>
                ))}
              </div>
            )}
            <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 16 }}>
              <button className="btn btn-ghost" onClick={() => setShowCliProxyPicker(false)}>
                {t("action.cancel", "取消")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
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
                background: `${colorMap[protocol] || "var(--accent)"}20`,
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

        {/* cli-proxy 平台只读提示（cpa-standalone-module s6）：models/base_url/api_key 由 provider 注入，
            platform 行字段留空，编辑表单内禁改 endpoints/auth/models 区。 */}
        {isCliProxyEditing && (
          <FormSection
            title={t("platform.cliProxy.inheritedTitle", "cli-proxy 继承字段（只读）")}
            desc={t("platform.cliProxy.inheritedDesc", "以下字段从 cli-proxy provider 继承，请到 cli-proxy 页编辑")}
          >
            <div style={{ display: "flex", flexDirection: "column", gap: 10, fontSize: 13 }}>
              <div>
                <div style={{ color: "var(--text-tertiary)", fontSize: 11, marginBottom: 4 }}>
                  {t("platform.cliProxy.provider", "Provider")}
                </div>
                <div>{cliProxyProvider?.name ?? "—"}</div>
              </div>
              <div>
                <div style={{ color: "var(--text-tertiary)", fontSize: 11, marginBottom: 4 }}>
                  {t("platform.cliProxy.wireProtocol", "入站协议")}
                </div>
                <div>{cliProxyProvider?.wire_protocol ?? "—"}</div>
              </div>
              <div>
                <div style={{ color: "var(--text-tertiary)", fontSize: 11, marginBottom: 4 }}>
                  {t("platform.cliProxy.baseUrl", "Base URL")}
                </div>
                <div style={{ wordBreak: "break-all" }}>{cliProxyProvider?.base_url ?? "—"}</div>
              </div>
              <div>
                <div style={{ color: "var(--text-tertiary)", fontSize: 11, marginBottom: 4 }}>
                  {t("platform.cliProxy.models", "模型")}
                </div>
                {cliProxyProvider && cliProxyProvider.models.length > 0 ? (
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                    {cliProxyProvider.models.map(m => (
                      <span key={m} style={{
                        padding: "2px 8px", borderRadius: "var(--radius-sm)",
                        background: "var(--bg-glass)", border: "1px solid var(--border)",
                        fontSize: 12,
                      }}>{m}</span>
                    ))}
                  </div>
                ) : <div>—</div>}
              </div>
            </div>
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

        {/* Protocol Endpoints（mock / 透传 / cli-proxy 平台隐藏，无可编辑上游） */}
        {!isMock && !isPassthrough && !isCliProxyEditing && (
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
            themeMode={themeMode}
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
