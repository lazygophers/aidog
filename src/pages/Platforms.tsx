import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { platformApi, type Platform, type Protocol, type ModelSlot } from "../services/api";

const PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: "anthropic", label: "Anthropic" },
  { value: "claude_code", label: "Claude Code" },
  { value: "openai", label: "OpenAI" },
  { value: "codex", label: "Codex" },
  { value: "glm", label: "GLM" },
  { value: "kimi", label: "Kimi" },
  { value: "minimax", label: "MiniMax" },
];

const DEFAULT_BASE_URLS: Partial<Record<Protocol, string>> = {
  glm: "https://open.bigmodel.cn/api/paas/v4",
  kimi: "https://api.moonshot.cn/v1",
  minimax: "https://api.minimaxi.com/v1",
  codex: "https://api.openai.com/v1",
  claude_code: "https://api.anthropic.com",
};

const ALL_DEFAULT_URLS = new Set(Object.values(DEFAULT_BASE_URLS));

const PROTOCOL_COLORS: Record<string, string> = {
  anthropic: "#D97757",
  claude_code: "#D97757",
  openai: "#10A37F",
  codex: "#10A37F",
  glm: "#3B5FEC",
  kimi: "#1783FF",
  minimax: "#6C5CE7",
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
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState<Platform | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [fetchError, setFetchError] = useState("");

  // Form state
  const [name, setName] = useState("");
  const [protocol, setProtocol] = useState<Protocol>("openai");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [models, setModels] = useState<Record<ModelSlot, string>>({
    default: "", sonnet: "", opus: "", haiku: "", gpt: "",
  });
  const [availableModels, setAvailableModels] = useState<string[]>([]);

  const handleProtocolChange = (newProtocol: Protocol) => {
    const oldDefault = DEFAULT_BASE_URLS[protocol];
    const newDefault = DEFAULT_BASE_URLS[newProtocol];
    if (!baseUrl || (oldDefault && baseUrl === oldDefault) || ALL_DEFAULT_URLS.has(baseUrl)) {
      setBaseUrl(newDefault || "");
    }
    setProtocol(newProtocol);
  };

  const load = async () => {
    setLoading(true);
    try {
      const list = await platformApi.list();
      setPlatforms(list || []);
    } catch (e) { console.error(e); }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const resetForm = () => {
    setName(""); setProtocol("openai"); setBaseUrl(""); setApiKey("");
    setModels({ default: "", sonnet: "", opus: "", haiku: "", gpt: "" });
    setAvailableModels([]);
    setEditing(null); setShowForm(false); setFetchError("");
  };

  const handleEdit = (p: Platform) => {
    setName(p.name); setProtocol(p.protocol); setBaseUrl(p.base_url); setApiKey(p.api_key);
    setModels({
      default: p.models.default ?? "",
      sonnet: p.models.sonnet ?? "",
      opus: p.models.opus ?? "",
      haiku: p.models.haiku ?? "",
      gpt: p.models.gpt ?? "",
    });
    setAvailableModels(p.available_models ?? []);
    setEditing(p); setShowForm(true); setFetchError("");
  };

  const handleModelChange = (slot: ModelSlot, value: string) => {
    setModels(prev => ({ ...prev, [slot]: value }));
  };

  /** 从下拉选择一个模型填入指定槽位 */
  const handleModelSelect = (slot: ModelSlot, value: string) => {
    setModels(prev => ({ ...prev, [slot]: value }));
  };

  /** 一键获取：获取模型列表 + 自动分类 + 持久化 */
  const handleFetchModels = async () => {
    if (!baseUrl || !apiKey) return;
    setFetching(true); setFetchError("");
    try {
      const modelIds = await platformApi.fetchModels(protocol, baseUrl, apiKey);
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

  /** 一键填充：把 default 模型填到所有空槽 */
  const handleFillAll = () => {
    const defaultModel = models.default.trim();
    if (!defaultModel) return;
    setModels(prev => {
      const next = { ...prev };
      for (const slot of MODEL_SLOTS) {
        if (slot.key !== "default" && !next[slot.key].trim()) {
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
    try {
      const modelsPayload = buildModelsPayload() as Platform["models"] | undefined;
      const availablePayload = availableModels.length > 0 ? availableModels : undefined;
      if (editing) {
        await platformApi.update({
          id: editing.id, name, protocol, base_url: baseUrl, api_key: apiKey,
          models: modelsPayload, available_models: availablePayload,
        });
      } else {
        await platformApi.create({
          name, protocol, base_url: baseUrl, api_key: apiKey,
          models: modelsPayload, available_models: availablePayload,
        });
      }
      resetForm(); load();
    } catch (e) { console.error(e); }
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

      {/* Add/Edit Form */}
      {showForm && (
        <div className="glass-surface animate-fade-in" style={{
          padding: 20,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}>
          <input className="input" placeholder={t("platform.name")} value={name}
            onChange={(e) => setName(e.target.value)} />
          <select className="input" value={protocol} onChange={(e) => handleProtocolChange(e.target.value as Protocol)}>
            {PROTOCOLS.map((p) => (
              <option key={p.value} value={p.value}>{p.label}</option>
            ))}
          </select>
          <input className="input" placeholder="Base URL" value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)} />
          <input className="input" type="password" placeholder="API Key" value={apiKey}
            onChange={(e) => setApiKey(e.target.value)} />

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
                  disabled={!baseUrl || !apiKey || fetching}
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
                    style={{ width: "100%" }}
                    placeholder={t(labelKey)}
                    value={models[key]}
                    onChange={(e) => handleModelChange(key, e.target.value)}
                    list={availableModels.length > 0 ? `model-list-${key}` : undefined}
                  />
                  {availableModels.length > 0 && (
                    <datalist id={`model-list-${key}`}>
                      {availableModels.map((m) => <option key={m} value={m} />)}
                    </datalist>
                  )}
                </div>
                {availableModels.length > 0 && (
                  <select
                    className="input"
                    style={{ width: 28, padding: "0 2px", fontSize: 10, appearance: "none",
                      cursor: "pointer", color: "var(--text-tertiary)",
                      background: "var(--bg-glass)", border: "1px solid var(--border)",
                      borderRadius: "var(--radius-sm)",
                    }}
                    value=""
                    onChange={(e) => { if (e.target.value) handleModelSelect(key, e.target.value); }}
                    title={t("platform.selectModel")}
                  >
                    <option value="">▼</option>
                    {availableModels.map((m) => <option key={m} value={m}>{m}</option>)}
                  </select>
                )}
              </div>
            ))}
          </div>

          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button className="btn" onClick={resetForm}>{t("action.cancel")}</button>
            <button className="btn btn-primary" onClick={handleSave}
              disabled={!name || !baseUrl || !apiKey}>
              {editing ? t("action.save") : t("action.create")}
            </button>
          </div>
        </div>
      )}

      {/* Platform List */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {platforms.length === 0 && !showForm && (
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
                    {p.protocol.toUpperCase()} · {p.base_url}
                  </div>
                  {configuredModels.length > 0 && (
                    <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 4 }}>
                      {configuredModels.map((m, mi) => (
                        <span key={mi} className="badge badge-muted" style={{ fontSize: 11, padding: "2px 6px" }}>
                          {m}
                        </span>
                      ))}
                    </div>
                  )}
                </div>

                <div style={{ display: "flex", gap: 4, flexShrink: 0 }}>
                  <button className="btn btn-ghost btn-icon" onClick={() => handleToggle(p)} title={p.enabled ? "Disable" : "Enable"}>
                    <span className={`status-dot ${p.enabled ? "status-dot-active" : "status-dot-inactive"}`} />
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
}
