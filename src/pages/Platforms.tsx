import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { platformApi, type Platform, type Protocol } from "../services/api";

const PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: "anthropic", label: "Anthropic" },
  { value: "openai", label: "OpenAI" },
  { value: "glm", label: "GLM" },
  { value: "kimi", label: "Kimi" },
];

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

  const load = async () => {
    setLoading(true);
    try {
      const list = await platformApi.list();
      setPlatforms(list || []);
    } catch (e) {
      console.error(e);
    }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const resetForm = () => {
    setName(""); setProtocol("openai"); setBaseUrl(""); setApiKey("");
    setEditing(null); setShowForm(false);
  };

  const handleEdit = (p: Platform) => {
    setName(p.name); setProtocol(p.protocol); setBaseUrl(p.base_url); setApiKey(p.api_key);
    setEditing(p); setShowForm(true);
  };

  const handleSave = async () => {
    try {
      if (editing) {
        await platformApi.update({
          id: editing.id, name, protocol, base_url: baseUrl, api_key: apiKey,
        });
      } else {
        await platformApi.create({ name, protocol, base_url: baseUrl, api_key: apiKey });
      }
      resetForm();
      load();
    } catch (e) {
      console.error(e);
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

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, maxWidth: 800, width: "100%" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h2 style={{ fontSize: 20, fontWeight: 600 }}>{t("page.platforms")}</h2>
        <button className="btn btn-primary" onClick={() => { resetForm(); setShowForm(true); }}>
          + {t("platform.add")}
        </button>
      </div>

      {showForm && (
        <div className="glass-surface" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 12 }}>
          <input className="input" placeholder={t("platform.name")} value={name}
            onChange={(e) => setName(e.target.value)} />
          <select className="input" value={protocol} onChange={(e) => setProtocol(e.target.value as Protocol)}>
            {PROTOCOLS.map((p) => (
              <option key={p.value} value={p.value}>{p.label}</option>
            ))}
          </select>
          <input className="input" placeholder="Base URL" value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)} />
          <input className="input" type="password" placeholder="API Key" value={apiKey}
            onChange={(e) => setApiKey(e.target.value)} />
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button className="btn" onClick={resetForm}>{t("action.cancel")}</button>
            <button className="btn btn-primary" onClick={handleSave}
              disabled={!name || !baseUrl || !apiKey}>
              {editing ? t("action.save") : t("action.create")}
            </button>
          </div>
        </div>
      )}

      {loading ? <p className="text-secondary">{t("status.loading")}</p> : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {platforms.length === 0 && <p className="text-tertiary">{t("platform.empty")}</p>}
          {platforms.map((p) => (
            <div key={p.id} className="glass-surface" style={{ padding: 16, display: "flex", alignItems: "center", gap: 12 }}>
              <button className="btn" onClick={() => handleToggle(p)}
                style={{ padding: "4px 8px", fontSize: 12, minWidth: 48,
                  background: p.enabled ? "var(--accent-subtle)" : "var(--bg-glass)" }}>
                {p.enabled ? "●" : "○"}
              </button>
              <div style={{ flex: 1 }}>
                <div style={{ fontWeight: 600 }}>{p.name}</div>
                <div className="text-secondary" style={{ fontSize: 12 }}>
                  {p.protocol.toUpperCase()} · {p.base_url}
                </div>
              </div>
              <button className="btn" style={{ padding: "4px 10px", fontSize: 12 }}
                onClick={() => handleEdit(p)}>✏️</button>
              <button className="btn" style={{ padding: "4px 10px", fontSize: 12 }}
                onClick={() => handleDelete(p.id)}>🗑️</button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
