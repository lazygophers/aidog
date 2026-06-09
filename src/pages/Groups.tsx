import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  groupDetailApi, groupApi, mappingApi, platformApi,
  type GroupDetail, type Platform, type RoutingMode,
} from "../services/api";

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

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, maxWidth: 800, width: "100%" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h2 style={{ fontSize: 20, fontWeight: 600 }}>{t("page.groups")}</h2>
        <button className="btn btn-primary" onClick={() => setShowForm(true)}>+ {t("group.add")}</button>
      </div>

      {showForm && (
        <div className="glass-surface" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 12 }}>
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

      {loading ? <p className="text-secondary">{t("status.loading")}</p> : (
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {details.length === 0 && <p className="text-tertiary">{t("group.empty")}</p>}
          {details.map(({ group, platforms: gps, model_mappings }) => (
            <div key={group.id} className="glass-surface" style={{ padding: 16 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
                <span style={{ fontWeight: 600, flex: 1 }}>{group.name}</span>
                <span className="text-secondary" style={{ fontSize: 12 }}>
                  {group.path} · {group.routing_mode === "failover" ? t("group.failover") : t("group.loadBalance")}
                </span>
                <button className="btn" style={{ padding: "4px 10px", fontSize: 12 }}
                  onClick={() => handleDeleteGroup(group.id)}>🗑️</button>
              </div>

              {/* Platforms */}
              {gps.length > 0 && (
                <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 6 }}>
                  {t("group.platforms")}: {gps.map((g) => g.platform.name).join(", ")}
                </div>
              )}

              {/* Model Mappings */}
              {model_mappings.length > 0 && (
                <div style={{ display: "flex", flexDirection: "column", gap: 4, marginBottom: 6 }}>
                  {model_mappings.map((m) => (
                    <div key={m.id} style={{
                      display: "flex", alignItems: "center", gap: 6, fontSize: 12,
                      padding: "4px 8px", borderRadius: "var(--radius-sm)",
                      background: "var(--accent-subtle)",
                    }}>
                      <span style={{ fontWeight: 600 }}>{m.source_model}</span>
                      <span className="text-tertiary">→</span>
                      <span>{m.target_model}</span>
                      <button style={{ marginLeft: "auto", background: "none", border: "none",
                        cursor: "pointer", color: "var(--text-tertiary)", fontSize: 12 }}
                        onClick={() => handleDeleteMapping(m.id)}>✕</button>
                    </div>
                  ))}
                </div>
              )}

              <button className="btn" style={{ fontSize: 12, padding: "4px 10px" }}
                onClick={() => setMappingGroupId(group.id)}>
                + {t("mapping.add")}
              </button>

              {mappingGroupId === group.id && (
                <div style={{ marginTop: 8, display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
                  <input className="input" style={{ flex: 1, minWidth: 120, fontSize: 12 }}
                    placeholder={t("mapping.source")} value={mSource}
                    onChange={(e) => setMSource(e.target.value)} />
                  <select className="input" style={{ fontSize: 12 }} value={mTargetPlatform}
                    onChange={(e) => setMTargetPlatform(e.target.value)}>
                    <option value="">{t("mapping.targetPlatform")}</option>
                    {platforms.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                  <input className="input" style={{ flex: 1, minWidth: 120, fontSize: 12 }}
                    placeholder={t("mapping.target")} value={mTargetModel}
                    onChange={(e) => setMTargetModel(e.target.value)} />
                  <button className="btn btn-primary" style={{ fontSize: 12, padding: "4px 10px" }}
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
