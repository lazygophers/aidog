import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { platformApi, type Platform, type Protocol } from "../services/api";

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

export function Platforms() {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState<Platform | null>(null);
  const [showForm, setShowForm] = useState(false);

  // Form state
  const [name, setName] = useState("");
  const [protocol, setProtocol] = useState<Protocol>("openai");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [modelInput, setModelInput] = useState("");
  const modelInputRef = useRef<HTMLInputElement>(null);

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
    setModels([]); setModelInput("");
    setEditing(null); setShowForm(false);
  };

  const handleEdit = (p: Platform) => {
    setName(p.name); setProtocol(p.protocol); setBaseUrl(p.base_url); setApiKey(p.api_key);
    setModels([...p.models]); setModelInput("");
    setEditing(p); setShowForm(true);
  };

  const handleAddModel = () => {
    const trimmed = modelInput.trim();
    if (trimmed && !models.includes(trimmed)) {
      setModels([...models, trimmed]);
      setModelInput("");
    }
  };

  const handleModelKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleAddModel();
    }
  };

  const handleRemoveModel = (idx: number) => {
    setModels(models.filter((_, i) => i !== idx));
  };

  const handleSave = async () => {
    try {
      if (editing) {
        await platformApi.update({
          id: editing.id, name, protocol, base_url: baseUrl, api_key: apiKey, models,
        });
      } else {
        await platformApi.create({ name, protocol, base_url: baseUrl, api_key: apiKey, models });
      }
      resetForm();
      load();
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
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <div style={{ fontSize: 13, fontWeight: 600, color: "var(--text-secondary)" }}>
              {t("platform.models")}
            </div>
            {models.length > 0 && (
              <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                {models.map((m, i) => (
                  <span key={i} className="badge badge-accent" style={{
                    display: "inline-flex",
                    alignItems: "center",
                    gap: 4,
                    padding: "4px 8px",
                    fontSize: 12,
                  }}>
                    {m}
                    <button
                      onClick={() => handleRemoveModel(i)}
                      style={{
                        background: "none", border: "none", cursor: "pointer",
                        color: "var(--accent)", padding: 0, lineHeight: 1,
                        display: "flex", alignItems: "center",
                      }}
                    >
                      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                        <path d="M2 2l6 6M8 2l-6 6" />
                      </svg>
                    </button>
                  </span>
                ))}
              </div>
            )}
            <div style={{ display: "flex", gap: 6 }}>
              <input
                ref={modelInputRef}
                className="input"
                style={{ flex: 1 }}
                placeholder={t("platform.modelPlaceholder")}
                value={modelInput}
                onChange={(e) => setModelInput(e.target.value)}
                onKeyDown={handleModelKeyDown}
              />
              <button
                className="btn"
                style={{ padding: "0 12px", fontSize: 13 }}
                onClick={handleAddModel}
                disabled={!modelInput.trim()}
              >
                +
              </button>
            </div>
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
                {/* Protocol Color Indicator */}
                <div style={{
                  width: 36, height: 36, borderRadius: "var(--radius-sm)",
                  display: "flex", alignItems: "center", justifyContent: "center",
                  background: `${color}15`,
                  border: `1px solid ${color}30`,
                  color: color,
                  fontSize: 11, fontWeight: 700,
                  flexShrink: 0,
                }}>
                  {p.protocol.slice(0, 2).toUpperCase()}
                </div>

                {/* Info */}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 600, fontSize: 14 }}>{p.name}</div>
                  <div className="text-secondary" style={{ fontSize: 12, marginTop: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {p.protocol.toUpperCase()} · {p.base_url}
                  </div>
                  {/* Model Tags */}
                  {p.models.length > 0 && (
                    <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 4 }}>
                      {p.models.slice(0, 5).map((m, mi) => (
                        <span key={mi} className="badge badge-muted" style={{ fontSize: 11, padding: "2px 6px" }}>
                          {m}
                        </span>
                      ))}
                      {p.models.length > 5 && (
                        <span className="badge badge-muted" style={{ fontSize: 11, padding: "2px 6px" }}>
                          +{p.models.length - 5}
                        </span>
                      )}
                    </div>
                  )}
                </div>

                {/* Actions */}
                <div style={{ display: "flex", gap: 4, flexShrink: 0 }}>
                  <button
                    className="btn btn-ghost btn-icon"
                    onClick={() => handleToggle(p)}
                    title={p.enabled ? "Disable" : "Enable"}
                  >
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
