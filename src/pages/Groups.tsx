import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  groupDetailApi, groupApi, mappingApi, platformApi,
  type GroupDetail, type Platform, type RoutingMode, type ModelSlot,
} from "../services/api";

const MODEL_SLOTS: ModelSlot[] = ["default", "sonnet", "opus", "haiku", "gpt"];

/** 从 PlatformModels 中提取所有非空模型名（去重） */
function allModelValues(models: Platform["models"]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const slot of MODEL_SLOTS) {
    const v = models[slot];
    if (v && !seen.has(v)) {
      seen.add(v);
      result.push(v);
    }
  }
  return result;
}

export function Groups() {
  const { t } = useTranslation();
  const [details, setDetails] = useState<GroupDetail[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [mappingGroupId, setMappingGroupId] = useState<string | null>(null);

  // Group form
  const [gName, setGName] = useState("");
  const [gPath, setGPath] = useState("/claude");
  const [gMode, setGMode] = useState<RoutingMode>("failover");

  // Mapping form
  const [mSource, setMSource] = useState("");
  const [mTargetPlatform, setMTargetPlatform] = useState("");
  const [mTargetModel, setMTargetModel] = useState("");

  const load = async () => {
    setLoading(true);
    try {
      const [d, p] = await Promise.all([groupDetailApi.list(), platformApi.list()]);
      setDetails(d || []);
      setPlatforms(p || []);
    } catch (e) { console.error(e); }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const handleCreateGroup = async () => {
    try {
      await groupApi.create({ name: gName, path: gPath, routing_mode: gMode });
      setGName(""); setGPath("/claude"); setGMode("failover"); setShowForm(false);
      load();
    } catch (e) { console.error(e); }
  };

  const handleDeleteGroup = async (id: string) => {
    try { await groupApi.delete(id); load(); } catch (e) { console.error(e); }
  };

  const handleAddMapping = async () => {
    if (!mappingGroupId || !mSource || !mTargetPlatform || !mTargetModel) return;
    try {
      await mappingApi.create({
        group_id: mappingGroupId,
        source_model: mSource,
        target_platform_id: mTargetPlatform,
        target_model: mTargetModel,
      });
      setMSource(""); setMTargetPlatform(""); setMTargetModel("");
      setMappingGroupId(null);
      load();
    } catch (e) { console.error(e); }
  };

  const handleDeleteMapping = async (id: string) => {
    try { await mappingApi.delete(id); load(); } catch (e) { console.error(e); }
  };

  // 获取当前选中平台的 models
  const selectedPlatform = platforms.find(p => p.id === mTargetPlatform);
  const availableModels = selectedPlatform ? allModelValues(selectedPlatform.models) : [];

  const handleTargetPlatformChange = (platformId: string) => {
    setMTargetPlatform(platformId);
    setMTargetModel(""); // 切换平台时重置 target model
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 720, width: "100%" }}>
      {/* Header */}
      <div className="section-header" style={{ justifyContent: "space-between" }}>
        <div>
          <div className="section-title">{t("page.groups")}</div>
          <div className="section-desc">
            {details.length > 0 ? `${details.length} ${t("nav.groups").toLowerCase()}` : t("group.empty")}
          </div>
        </div>
        <button className="btn btn-primary" onClick={() => setShowForm(true)}>
          + {t("group.add")}
        </button>
      </div>

      {/* Add Group Form */}
      {showForm && (
        <div className="glass-surface animate-fade-in" style={{
          padding: 20,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}>
          <input className="input" placeholder={t("group.name")} value={gName}
            onChange={(e) => setGName(e.target.value)} />
          <input className="input" placeholder="Path (e.g. /claude)" value={gPath}
            onChange={(e) => setGPath(e.target.value)} />
          <select className="input" value={gMode} onChange={(e) => setGMode(e.target.value as RoutingMode)}>
            <option value="failover">{t("group.failover")}</option>
            <option value="load_balance">{t("group.loadBalance")}</option>
          </select>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button className="btn" onClick={() => setShowForm(false)}>{t("action.cancel")}</button>
            <button className="btn btn-primary" onClick={handleCreateGroup}
              disabled={!gName || !gPath}>{t("action.create")}</button>
          </div>
        </div>
      )}

      {/* Group List */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {details.length === 0 && !showForm && (
            <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
              <div className="text-tertiary" style={{ fontSize: 13 }}>{t("group.empty")}</div>
            </div>
          )}
          {details.map(({ group, platforms: gps, model_mappings }, i) => (
            <div
              key={group.id}
              className="card-item animate-fade-in"
              style={{ animationDelay: `${i * 60}ms` }}
            >
              {/* Group Header */}
              <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 10 }}>
                <div style={{
                  width: 32, height: 32, borderRadius: "var(--radius-sm)",
                  display: "flex", alignItems: "center", justifyContent: "center",
                  background: "var(--accent-subtle)",
                  color: "var(--accent)",
                  fontSize: 13, fontWeight: 700,
                  flexShrink: 0,
                }}>
                  {group.path.slice(0, 3)}
                </div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 600, fontSize: 14 }}>{group.name}</div>
                  <div className="text-secondary" style={{ fontSize: 12, display: "flex", gap: 8, marginTop: 1 }}>
                    <span>{group.path}</span>
                    <span className="badge badge-muted" style={{ padding: "0 6px" }}>
                      {group.routing_mode === "failover" ? t("group.failover") : t("group.loadBalance")}
                    </span>
                  </div>
                </div>
                <button className="btn btn-ghost btn-icon btn-danger" onClick={() => handleDeleteGroup(group.id)}>
                  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                  </svg>
                </button>
              </div>

              {/* Platforms */}
              {gps.length > 0 && (
                <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 8 }}>
                  {gps.map((g) => (
                    <span key={g.platform.id} className="badge badge-accent">
                      {g.platform.name}
                    </span>
                  ))}
                </div>
              )}

              {/* Model Mappings */}
              {model_mappings.length > 0 && (
                <div style={{ display: "flex", flexDirection: "column", gap: 4, marginBottom: 8 }}>
                  {model_mappings.map((m) => (
                    <div key={m.id} style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      fontSize: 12,
                      padding: "6px 10px",
                      borderRadius: "var(--radius-sm)",
                      background: "var(--bg-glass)",
                      border: "1px solid var(--border)",
                    }}>
                      <span style={{ fontWeight: 600, color: "var(--accent)" }}>{m.source_model}</span>
                      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round">
                        <path d="M2 6h8M8 4l2 2-2 2" />
                      </svg>
                      <span style={{ flex: 1 }}>{m.target_model}</span>
                      <button className="btn btn-ghost btn-icon" style={{ width: 24, height: 24, minWidth: 24, padding: 0 }}
                        onClick={() => handleDeleteMapping(m.id)}>
                        <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round">
                          <path d="M2 2l6 6M8 2l-6 6" />
                        </svg>
                      </button>
                    </div>
                  ))}
                </div>
              )}

              {/* Add Mapping */}
              <button className="btn btn-ghost" style={{ fontSize: 12, gap: 4, padding: "4px 8px", color: "var(--text-secondary)" }}
                onClick={() => setMappingGroupId(group.id)}>
                <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                  <path d="M6 2v8M2 6h8" />
                </svg>
                {t("mapping.add")}
              </button>

              {mappingGroupId === group.id && (
                <div className="animate-fade-in" style={{
                  marginTop: 10,
                  paddingTop: 10,
                  borderTop: "1px solid var(--border)",
                  display: "flex",
                  gap: 8,
                  alignItems: "center",
                  flexWrap: "wrap",
                }}>
                  <input className="input" style={{ flex: 1, minWidth: 100, fontSize: 12 }}
                    placeholder={t("mapping.source")} value={mSource}
                    onChange={(e) => setMSource(e.target.value)} />
                  <select className="input" style={{ fontSize: 12, width: 140 }} value={mTargetPlatform}
                    onChange={(e) => handleTargetPlatformChange(e.target.value)}>
                    <option value="">{t("mapping.targetPlatform")}</option>
                    {platforms.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                  {availableModels.length > 0 ? (
                    <select className="input" style={{ flex: 1, minWidth: 100, fontSize: 12 }} value={mTargetModel}
                      onChange={(e) => setMTargetModel(e.target.value)}>
                      <option value="">{t("mapping.target")}</option>
                      {availableModels.map((m) => <option key={m} value={m}>{m}</option>)}
                    </select>
                  ) : (
                    <input className="input" style={{ flex: 1, minWidth: 100, fontSize: 12 }}
                      placeholder={t("mapping.target")} value={mTargetModel}
                      onChange={(e) => setMTargetModel(e.target.value)} />
                  )}
                  <button className="btn btn-primary" style={{ fontSize: 12, padding: "6px 12px" }}
                    onClick={handleAddMapping}
                    disabled={!mSource || !mTargetPlatform || !mTargetModel}>
                    {t("action.create")}
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
