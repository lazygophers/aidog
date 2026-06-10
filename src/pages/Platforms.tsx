import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { platformApi, settingsApi, type Platform, type Protocol, type ModelSlot, type PlatformEndpoint, type ClientType, type PlatformUsageStats } from "../services/api";
import { ModelTestPanel } from "./ModelTestPanel";

const PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: "anthropic", label: "Anthropic" },
  { value: "openai", label: "OpenAI" },
  { value: "codex", label: "Codex" },
  { value: "gemini", label: "Gemini" },
  { value: "glm", label: "GLM" },
  { value: "kimi", label: "Kimi" },
  { value: "minimax", label: "MiniMax" },
  { value: "bailian", label: "百炼" },
];

/** Endpoint 协议只支持这三种标准格式 */
const ENDPOINT_PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "gemini", label: "Gemini" },
];

/** 客户端模拟选项：用于通过上游客户端校验 */
const CLIENT_TYPES: { value: ClientType; label: string; desc: string }[] = [
  { value: "default", label: "Default", desc: "无模拟" },
  { value: "claude_code", label: "Claude Code", desc: "模拟 claude-cli" },
  { value: "codex_cli", label: "Codex CLI", desc: "模拟 codex_cli_rs" },
  { value: "cursor", label: "Cursor", desc: "模拟 Cursor IDE" },
  { value: "windsurf", label: "Windsurf", desc: "模拟 Windsurf IDE" },
];


/** 内置平台默认端点：每个平台支持的协议及其 base URL
 * URL 为不含 adapter 路径前缀的基础地址，proxy 会拼接 adapter 路径
 * 来源：各平台官方文档 */
const DEFAULT_ENDPOINTS: Partial<Record<Protocol, PlatformEndpoint[]>> = {
  anthropic: [
    { protocol: "anthropic", base_url: "https://api.anthropic.com" },
  ],
  openai: [
    { protocol: "openai", base_url: "https://api.openai.com" },
  ],
  codex: [
    { protocol: "openai", base_url: "https://api.openai.com" },
  ],
  glm: [
    { protocol: "openai", base_url: "https://open.bigmodel.cn/api/paas/v4" },
    { protocol: "anthropic", base_url: "https://open.bigmodel.cn/api/anthropic" },
  ],
  bailian: [
    { protocol: "openai", base_url: "https://dashscope.aliyuncs.com/compatible-mode" },
  ],
  minimax: [
    { protocol: "openai", base_url: "https://api.minimaxi.com" },
    { protocol: "anthropic", base_url: "https://api.minimaxi.com/anthropic" },
  ],
  kimi: [
    { protocol: "openai", base_url: "https://api.moonshot.cn" },
  ],
  gemini: [
    { protocol: "gemini", base_url: "https://generativelanguage.googleapis.com" },
  ],
};

const PROTOCOL_LABELS: Record<Protocol, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
  glm: "GLM",
  kimi: "Kimi",
  minimax: "MiniMax",
  codex: "Codex",
  bailian: "百炼",
  gemini: "Gemini",
};

const DEFAULT_NAMES = new Set(Object.values(PROTOCOL_LABELS));

const PROTOCOL_COLORS: Record<string, string> = {
  anthropic: "#D97757",
  openai: "#10A37F",
  codex: "#10A37F",
  gemini: "#4285F4",
  glm: "#3B5FEC",
  kimi: "#1783FF",
  minimax: "#6C5CE7",
  bailian: "#FF6A00",
};

const MODEL_SLOTS: { key: ModelSlot; labelKey: string }[] = [
  { key: "default", labelKey: "platform.modelDefault" },
  { key: "sonnet", labelKey: "platform.modelSonnet" },
  { key: "opus", labelKey: "platform.modelOpus" },
  { key: "haiku", labelKey: "platform.modelHaiku" },
  { key: "gpt", labelKey: "platform.modelGpt" },
];

/** 从 PlatformModels 中提取所有非空值（去重） */
function allModelValues(models: Platform["models"]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const slot of MODEL_SLOTS) {
    const v = models[slot.key];
    if (v && !seen.has(v)) {
      seen.add(v);
      result.push(v);
    }
  }
  return result;
}

/** 根据模型名模式自动分配到槽位 */
function autoCategorize(modelIds: string[]): Record<ModelSlot, string> {
  const result: Record<ModelSlot, string> = {
    default: "", sonnet: "", opus: "", haiku: "", gpt: "",
  };
  const patterns: { slot: ModelSlot; test: (id: string) => boolean }[] = [
    { slot: "opus", test: (id) => /opus/i.test(id) },
    { slot: "sonnet", test: (id) => /sonnet/i.test(id) },
    { slot: "haiku", test: (id) => /haiku/i.test(id) },
    { slot: "gpt", test: (id) => /gpt/i.test(id) && !/mini/i.test(id) },
  ];
  const assigned = new Set<string>();
  for (const { slot, test } of patterns) {
    for (const id of modelIds) {
      if (test(id) && !assigned.has(id)) {
        result[slot] = id;
        assigned.add(id);
      }
    }
  }
  const first = modelIds.find(id => !assigned.has(id)) ?? modelIds[0];
  if (first && !result.default) result.default = first;
  return result;
}

export function Platforms() {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [usageMap, setUsageMap] = useState<Record<string, PlatformUsageStats>>({});
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState<Platform | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState("");
  const [saveError, setSaveError] = useState("");
const [testingPlatform, setTestingPlatform] = useState<Platform | null>(null);
  const [showKey, setShowKey] = useState(false);

  // Form state
  const [name, setName] = useState("OpenAI");
  const [protocol, setProtocol] = useState<Protocol>("openai");
  const [apiKey, setApiKey] = useState("");
  const [models, setModels] = useState<Record<ModelSlot, string>>({
    default: "", sonnet: "", opus: "", haiku: "", gpt: "",
  });
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [endpoints, setEndpoints] = useState<PlatformEndpoint[]>([]);
  const [activeDropdown, setActiveDropdown] = useState<ModelSlot | null>(null);
  const [showClaudeConfig, setShowClaudeConfig] = useState(false);
  const [claudeConfigJson, setClaudeConfigJson] = useState("");
  const [globalClaudeConfig, setGlobalClaudeConfig] = useState<Record<string, any>>({});

  /** 从 endpoints 中推导主 base_url（匹配主协议的 endpoint，否则取第一个） */
  const getPrimaryBaseUrl = (proto: Protocol, eps: PlatformEndpoint[]): string => {
    const primary = eps.find(ep => ep.protocol === proto);
    if (primary) return primary.base_url;
    return eps[0]?.base_url || "";
  };

  const handleProtocolChange = (newProtocol: Protocol) => {
    // Auto-fill name with protocol label if empty or still at a default name
    if (!name.trim() || DEFAULT_NAMES.has(name)) {
      setName(PROTOCOL_LABELS[newProtocol]);
    }
    // Auto-fill endpoints from defaults
    const defaultEps = DEFAULT_ENDPOINTS[newProtocol];
    if (defaultEps) {
      setEndpoints(defaultEps.map(ep => ({ ...ep })));
    } else {
      setEndpoints([]);
    }
    setProtocol(newProtocol);
  };

  const load = async () => {
    setLoading(true);
    try {
      const list = await platformApi.list();
      setPlatforms(list || []);
      // Batch load usage stats
      const statsMap: Record<string, PlatformUsageStats> = {};
      await Promise.all((list || []).map(async (p) => {
        try {
          const s = await platformApi.usageStats(p.id);
          if (s && s.total_requests > 0) statsMap[p.id] = s;
        } catch { /* ignore */ }
      }));
      setUsageMap(statsMap);
    } catch (e) { console.error(e); }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const resetForm = () => {
    setName(""); setProtocol("openai"); setApiKey("");
    setModels({ default: "", sonnet: "", opus: "", haiku: "", gpt: "" });
    setAvailableModels([]); setEndpoints([]);
    setEditing(null); setShowForm(false); setFetchError(""); setSaveError("");
    setShowClaudeConfig(false); setClaudeConfigJson("");
  };

  const handleEdit = async (p: Platform) => {
    setName(p.name); setProtocol(p.protocol); setApiKey(p.api_key);
    setModels({
      default: p.models.default ?? "",
      sonnet: p.models.sonnet ?? "",
      opus: p.models.opus ?? "",
      haiku: p.models.haiku ?? "",
      gpt: p.models.gpt ?? "",
    });
    setAvailableModels(p.available_models ?? []);
    setEndpoints(p.endpoints ?? []);
    setEditing(p); setShowForm(true); setFetchError(""); setSaveError("");
    setShowClaudeConfig(false); setClaudeConfigJson("");

    // Load global + platform Claude Code config
    try {
      const [globalResult, platformResult] = await Promise.all([
        settingsApi.get("global", "claude_code"),
        settingsApi.get(`platform:${p.id}`, "claude_code"),
      ]);
      const gv = (globalResult as Record<string, any>) ?? {};
      const pv = (platformResult as Record<string, any>) ?? {};
      setGlobalClaudeConfig(gv);
      setClaudeConfigJson(JSON.stringify({ ...gv, ...pv }, null, 2));
    } catch (e) { console.error(e); }
  };

  const handleModelChange = (slot: ModelSlot, value: string) => {
    setModels(prev => ({ ...prev, [slot]: value }));
  };

  /** 从下拉选择一个模型填入指定槽位 */
  const handleModelSelect = (slot: ModelSlot, value: string) => {
    setModels(prev => ({ ...prev, [slot]: value }));
  };

  /** 一键获取：获取模型列表 + 自动分类 + 持久化
   *  默认使用 OpenAI 协议 endpoint，回退到主协议 endpoint */
  const handleFetchModels = async () => {
    const openaiEp = endpoints.find(ep => ep.protocol === "openai");
    const fetchUrl = openaiEp?.base_url || getPrimaryBaseUrl(protocol, endpoints);
    if (!fetchUrl || !apiKey) return;
    setFetching(true); setFetchError("");
    try {
      const fetchProtocol: Protocol = openaiEp ? "openai" : protocol;
      const modelIds = await platformApi.fetchModels(fetchProtocol, fetchUrl, apiKey);
      if (modelIds.length === 0) {
        setFetchError(t("platform.fetchEmpty"));
      } else {
        setAvailableModels(modelIds);
        const categorized = autoCategorize(modelIds);
        setModels(categorized);
      }
    } catch (e: any) {
      setFetchError(e.toString());
    }
    setFetching(false);
  };

  /** 一键填充：把 default 模型填到所有槽位（覆盖已有值） */
  const handleFillAll = () => {
    const defaultModel = models.default.trim();
    if (!defaultModel) return;
    setModels(prev => {
      const next = { ...prev };
      for (const slot of MODEL_SLOTS) {
        if (slot.key !== "default") {
          next[slot.key] = defaultModel;
        }
      }
      return next;
    });
  };

  const buildModelsPayload = () => {
    const result: Record<string, string | undefined> = {};
    let hasAny = false;
    for (const slot of MODEL_SLOTS) {
      const v = models[slot.key].trim();
      if (v) { result[slot.key] = v; hasAny = true; }
      else { result[slot.key] = undefined; }
    }
    return hasAny ? result : undefined;
  };

  const handleSave = async () => {
    setSaveError("");
    try {
      const modelsPayload = buildModelsPayload() as Platform["models"] | undefined;
      const availablePayload = availableModels.length > 0 ? availableModels : undefined;
      const baseUrl = getPrimaryBaseUrl(protocol, endpoints);
      let savedId: string | undefined;
      if (editing) {
        await platformApi.update({
          id: editing.id, name, protocol, base_url: baseUrl, api_key: apiKey,
          models: modelsPayload, available_models: availablePayload,
          endpoints: endpoints.length > 0 ? endpoints : undefined,
        });
        savedId = editing.id;
      } else {
        const created = await platformApi.create({
          name, protocol, base_url: baseUrl, api_key: apiKey,
          models: modelsPayload, available_models: availablePayload,
          endpoints: endpoints.length > 0 ? endpoints : undefined,
        });
        savedId = created.id;
      }

      // Save Claude Code config overrides for this platform
      if (savedId && showClaudeConfig && claudeConfigJson.trim()) {
        try {
          const merged = JSON.parse(claudeConfigJson);
          const diff: Record<string, any> = {};
          for (const [k, v] of Object.entries(merged)) {
            if (JSON.stringify(v) !== JSON.stringify(globalClaudeConfig[k])) {
              diff[k] = v;
            }
          }
          if (Object.keys(diff).length > 0) {
            await settingsApi.set(`platform:${savedId}`, "claude_code", diff);
          } else {
            await settingsApi.delete(`platform:${savedId}`, "claude_code");
          }
        } catch (e) { /* ignore JSON parse errors for config */ }
      }

      resetForm(); load();
    } catch (e: any) {
      const msg = e?.toString() || "Unknown error";
      console.error(msg);
      setSaveError(msg);
    }
  };

  const handleDelete = async (id: string) => {
    try { await platformApi.delete(id); load(); } catch (e) { console.error(e); }
  };

  const handleToggle = async (p: Platform) => {
    try {
      await platformApi.update({ id: p.id, enabled: !p.enabled });
      load();
    } catch (e) { console.error(e); }
  };

  // ── Edit / Add form (full page, no list) ──
  if (showForm) {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 720, width: "100%" }}>
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
              <div className="section-desc">{editing.protocol.toUpperCase()} · {getPrimaryBaseUrl(editing.protocol, editing.endpoints ?? []) || editing.base_url}</div>
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn" onClick={resetForm}>{t("action.cancel")}</button>
            <button className="btn btn-primary" onClick={handleSave}
              disabled={!name || endpoints.length === 0 || !apiKey}>
              {editing ? t("action.save") : t("action.create")}
            </button>
          </div>
        </div>

        <div className="glass-surface animate-fade-in" style={{
          padding: 20,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}>
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
            <select className="input" value={protocol} onChange={(e) => handleProtocolChange(e.target.value as Protocol)}>
              {PROTOCOLS.map((p) => (
                <option key={p.value} value={p.value}>{p.label}</option>
              ))}
            </select>
          )}

          {/* Protocol Endpoints */}
          <div style={{
            display: "flex", flexDirection: "column", gap: 6,
            padding: "8px 0",
          }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-secondary)" }}>
                {t("platform.endpoints", "Protocol Endpoints")}
              </div>
              <button
                type="button"
                className="btn btn-ghost"
                style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
                onClick={() => setEndpoints([...endpoints, { protocol: "openai", base_url: "" }])}
              >
                + {t("platform.addEndpoint", "Add Endpoint")}
              </button>
            </div>
            <div style={{ fontSize: 11, color: "var(--text-tertiary)", lineHeight: 1.4 }}>
              {t("platform.endpointsHint", "Additional protocols this platform supports with different base URLs")}
            </div>
            {endpoints.length === 0 && (
              <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "4px 0", fontStyle: "italic" }}>
                {t("platform.noEndpoints", "No additional endpoints")}
              </div>
            )}
            {endpoints.map((ep, idx) => (
              <div key={idx} style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <select
                  className="input"
                  style={{ width: 120, flexShrink: 0 }}
                  value={ep.protocol}
                  onChange={(e) => {
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], protocol: e.target.value as Protocol };
                    setEndpoints(next);
                  }}
                >
                  {ENDPOINT_PROTOCOLS.map((p) => (
                    <option key={p.value} value={p.value}>{p.label}</option>
                  ))}
                </select>
                <input
                  className="input"
                  style={{ flex: 1 }}
                  placeholder="Endpoint Base URL"
                  value={ep.base_url}
                  onChange={(e) => {
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], base_url: e.target.value };
                    setEndpoints(next);
                  }}
                />
                <select
                  className="input"
                  style={{ width: 120, flexShrink: 0 }}
                  value={ep.client_type || "default"}
                  onChange={(e) => {
                    const next = [...endpoints];
                    next[idx] = { ...next[idx], client_type: e.target.value as ClientType };
                    setEndpoints(next);
                  }}
                  title={t("platform.clientType", "客户端模拟")}
                >
                  {CLIENT_TYPES.map((c) => (
                    <option key={c.value} value={c.value}>{c.label}</option>
                  ))}
                </select>
                <button
                  type="button"
                  className="btn btn-ghost btn-icon btn-danger"
                  style={{ flexShrink: 0 }}
                  onClick={() => setEndpoints(endpoints.filter((_, i) => i !== idx))}
                >
                  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                  </svg>
                </button>
              </div>
            ))}
          </div>

          {/* API Key with show/copy */}
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <input
              className="input"
              type={showKey ? "text" : "password"}
              placeholder="API Key"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              style={{ flex: 1 }}
            />
            <button
              type="button"
              className="btn btn-ghost btn-icon"
              title={showKey ? "Hide key" : "Show key"}
              onClick={() => setShowKey(!showKey)}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                {showKey ? (
                  <>
                    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
                    <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
                    <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
                    <line x1="1" y1="1" x2="23" y2="23" />
                  </>
                ) : (
                  <>
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                    <circle cx="12" cy="12" r="3" />
                  </>
                )}
              </svg>
            </button>
            {editing && apiKey && (
              <button
                type="button"
                className="btn btn-ghost btn-icon"
                title="Copy key"
                onClick={() => navigator.clipboard.writeText(apiKey)}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              </button>
            )}
          </div>

          {/* Models Configuration */}
          <div style={{
            display: "flex", flexDirection: "column", gap: 6,
            padding: "12px 0 4px",
            borderTop: "1px solid var(--border)",
          }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-secondary)" }}>
                {t("platform.models")}
              </div>
              <div style={{ display: "flex", gap: 6 }}>
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--text-secondary)" }}
                  onClick={handleFillAll}
                  disabled={!models.default.trim()}
                  title={t("platform.fillAllHint")}
                >
                  {t("platform.fillAll")}
                </button>
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 12, gap: 4, padding: "4px 10px", color: "var(--accent)" }}
                  onClick={handleFetchModels}
                  disabled={!apiKey || endpoints.length === 0 || fetching}
                >
                  {fetching ? t("status.loading") : t("platform.fetchModels")}
                </button>
              </div>
            </div>
            {fetchError && (
              <div style={{ fontSize: 12, color: "var(--danger, #e55)", padding: "2px 0" }}>
                {fetchError}
              </div>
            )}
            {MODEL_SLOTS.map(({ key, labelKey }) => (
              <div key={key} style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{
                  fontSize: 12, fontWeight: 500, color: "var(--text-tertiary)",
                  width: 56, textAlign: "right", flexShrink: 0,
                }}>
                  {t(labelKey)}
                </span>
                <div style={{ position: "relative", flex: 1 }}>
                  <input
                    className="input"
                    style={{ width: "100%", paddingRight: availableModels.length > 0 ? 28 : undefined }}
                    placeholder={t(labelKey)}
                    value={models[key]}
                    onChange={(e) => handleModelChange(key, e.target.value)}
                  />
                  {availableModels.length > 0 && (
                    <button
                      type="button"
                      className="btn btn-ghost btn-icon"
                      style={{
                        position: "absolute", right: 2, top: "50%", transform: "translateY(-50%)",
                        width: 24, height: 24, minWidth: 24, padding: 0,
                        color: "var(--text-tertiary)", cursor: "pointer",
                      }}
                      onClick={() => setActiveDropdown(activeDropdown === key ? null : key)}
                      title={t("platform.selectModel")}
                    >
                      ▾
                    </button>
                  )}
                  {/* 自定义下拉列表 — 主题化 */}
                  {activeDropdown === key && availableModels.length > 0 && (
                    <>
                      <div
                        style={{ position: "fixed", inset: 0, zIndex: 99 }}
                        onClick={() => setActiveDropdown(null)}
                      />
                      <div
                        className="glass-elevated"
                        style={{
                          position: "absolute",
                          top: "100%",
                          left: 0,
                          right: 0,
                          marginTop: 4,
                          maxHeight: 200,
                          overflowY: "auto",
                          zIndex: 100,
                          padding: 4,
                          animation: "fadeIn 150ms ease both",
                        }}
                      >
                        {availableModels.map((m) => (
                          <button
                            key={m}
                            type="button"
                            className="btn btn-ghost"
                            style={{
                              width: "100%",
                              justifyContent: "flex-start",
                              padding: "6px 10px",
                              fontSize: 12,
                              fontWeight: models[key] === m ? 600 : 400,
                              color: models[key] === m ? "var(--accent)" : "var(--text-primary)",
                              background: models[key] === m ? "var(--accent-subtle)" : "transparent",
                              borderRadius: "var(--radius-sm)",
                            }}
                            onClick={() => {
                              handleModelSelect(key, m);
                              setActiveDropdown(null);
                            }}
                          >
                            {m}
                          </button>
                        ))}
                      </div>
                    </>
                  )}
                </div>
              </div>
            ))}
          </div>

          {/* Claude Code Config */}
          {editing && (
            <div style={{
              borderTop: "1px solid var(--border)",
              paddingTop: 8,
            }}>
              <button
                type="button"
                className="btn btn-ghost"
                style={{
                  width: "100%",
                  justifyContent: "space-between",
                  fontSize: 12,
                  padding: "6px 4px",
                  color: "var(--text-secondary)",
                }}
                onClick={() => setShowClaudeConfig(!showClaudeConfig)}
              >
                <span style={{ fontWeight: 600 }}>{t("settings.claudeCodeConfig")}</span>
                <span style={{ opacity: 0.5 }}>{showClaudeConfig ? "▾" : "▸"}</span>
              </button>
              {showClaudeConfig && (
                <div className="animate-fade-in" style={{ marginTop: 6 }}>
                  <textarea
                    className="input"
                    style={{
                      fontFamily: '"SF Mono", "Fira Code", monospace',
                      fontSize: 12,
                      lineHeight: 1.6,
                      minHeight: 180,
                      resize: "vertical",
                      whiteSpace: "pre",
                    }}
                    value={claudeConfigJson}
                    onChange={(e) => setClaudeConfigJson(e.target.value)}
                    spellCheck={false}
                  />
                  <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4, lineHeight: 1.5 }}>
                    {t("settings.platformConfigHint")}
                  </div>
                  {(() => {
                    try {
                      const merged = JSON.parse(claudeConfigJson);
                      const overridden = Object.keys(merged).filter(
                        k => JSON.stringify(merged[k]) !== JSON.stringify(globalClaudeConfig[k]),
                      );
                      return overridden.length > 0 ? (
                        <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 4 }}>
                          {overridden.map(k => (
                            <span key={k} className="badge badge-accent" style={{ fontSize: 10 }}>
                              {k}
                            </span>
                          ))}
                        </div>
                      ) : (
                        <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4 }}>
                          {t("settings.allAligned")}
                        </div>
                      );
                    } catch { return null; }
                  })()}
                </div>
              )}
            </div>
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

  // ── List view ──
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 720, width: "100%" }}>
      {/* Header */}
      <div className="section-header" style={{ justifyContent: "space-between" }}>
        <div>
          <div className="section-title">{t("page.platforms")}</div>
          <div className="section-desc">
            {platforms.length > 0 ? `${platforms.filter(p => p.enabled).length} / ${platforms.length} active` : t("platform.empty")}
          </div>
        </div>
        <button className="btn btn-primary" onClick={() => { resetForm(); setShowForm(true); }}>
          + {t("platform.add")}
        </button>
      </div>

      {/* Platform List */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {platforms.length === 0 && (
            <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
              <div className="text-tertiary" style={{ fontSize: 13 }}>{t("platform.empty")}</div>
            </div>
          )}
          {platforms.map((p, i) => {
            const color = PROTOCOL_COLORS[p.protocol] || "var(--accent)";
            const configuredModels = allModelValues(p.models);
            return (
              <div
                key={p.id}
                className="card-item"
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 14,
                  animationDelay: `${i * 50}ms`,
                  opacity: p.enabled ? 1 : 0.5,
                }}
              >
                <div style={{
                  width: 36, height: 36, borderRadius: "var(--radius-sm)",
                  display: "flex", alignItems: "center", justifyContent: "center",
                  background: `${color}15`,
                  border: `1px solid ${color}30`,
                  color: color, fontSize: 11, fontWeight: 700, flexShrink: 0,
                }}>
                  {p.protocol.slice(0, 2).toUpperCase()}
                </div>

                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 600, fontSize: 14 }}>{p.name}</div>
                  <div className="text-secondary" style={{ fontSize: 12, marginTop: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {p.protocol.toUpperCase()} · {getPrimaryBaseUrl(p.protocol, p.endpoints ?? []) || p.base_url}
                  </div>
                  {p.endpoints && p.endpoints.length > 0 && (
                    <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 3 }}>
                      {p.endpoints.map((ep, ei) => (
                        <span key={ei} className="badge badge-muted" style={{ fontSize: 10, padding: "1px 6px", opacity: 0.8 }}>
                          {PROTOCOL_LABELS[ep.protocol] || ep.protocol}
                        </span>
                      ))}
                    </div>
                  )}
                  {configuredModels.length > 0 && (
                    <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 4 }}>
                      {configuredModels.map((m, mi) => (
                        <span key={mi} className="badge badge-muted" style={{ fontSize: 11, padding: "2px 6px" }}>
                          {m}
                        </span>
                      ))}
                    </div>
                  )}
                  {usageMap[p.id] && (() => {
                    const u = usageMap[p.id];
                    const total = u.total_input_tokens + u.total_output_tokens;
                    return (
                      <div style={{ display: "flex", gap: 8, marginTop: 4, fontSize: 11, color: "var(--text-tertiary)" }}>
                        <span>{formatTokens(total)} tokens</span>
                        <span>·</span>
                        <span>↑{formatTokens(u.total_input_tokens)} ↓{formatTokens(u.total_output_tokens)}</span>
                        {u.cache_rate > 0 && (
                          <>
                            <span>·</span>
                            <span style={{ color: "var(--color-success, #34c759)" }}>cache {u.cache_rate.toFixed(1)}%</span>
                          </>
                        )}
                        <span>·</span>
                        <span>{u.total_requests} req</span>
                      </div>
                    );
                  })()}
                </div>

                <div style={{ display: "flex", gap: 6, flexShrink: 0, alignItems: "center" }}>
                  <div
                    className={`toggle ${p.enabled ? "active" : ""}`}
                    style={{ cursor: "pointer" }}
                    onClick={() => handleToggle(p)}
                    title={p.enabled ? "Disable" : "Enable"}
                  />
                  <button className="btn btn-ghost btn-icon" onClick={() => setTestingPlatform(p)} title="Test">
                    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M7 1v4M7 9v4M1 7h4M9 7h4" />
                    </svg>
                  </button>
                  <button className="btn btn-ghost btn-icon" onClick={() => handleEdit(p)}>
                    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M10 2l2 2-7 7H3v-2l7-7z" />
                    </svg>
                  </button>
                  <button className="btn btn-ghost btn-icon btn-danger" onClick={() => handleDelete(p.id)}>
                    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                    </svg>
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );

      {testingPlatform !== null && (
        <ModelTestPanel platform={testingPlatform as Platform} onClose={() => setTestingPlatform(null)} />
      )}
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return `${n}`;
}