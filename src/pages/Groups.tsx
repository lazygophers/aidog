import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  groupDetailApi, groupApi, mappingApi, platformApi, groupUsageApi,
  type GroupDetail, type Platform, type RoutingMode, type ModelSlot, type PlatformUsageStats,
} from "../services/api";

const MODEL_SLOTS: ModelSlot[] = ["default", "sonnet", "opus", "haiku", "gpt"];

/** Extract all non-empty model names (deduplicated) */
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

/** Build the `claude` CLI invocation for a given group settings file */
function buildClaudeCommand(settingsName: string): string {
  return [
    "claude",
    "--brief",
    "--dangerously-skip-permissions",
    "--settings",
    `~/.aidog/settings.${settingsName}.json`,
  ].join(" ");
}

// ─── Design tokens ───
const F = { title: 20, label: 15, body: 15, hint: 13, small: 12 } as const;
const S = { gap: 18, pad: 28, inputPad: "10px 14px", btnPad: "8px 18px", btnIcon: 34 } as const;

/** Copy text to clipboard with a brief visual feedback */
function CopyButton({ text, title, size = 14 }: { text: string; title?: string; size?: number }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  return (
    <button
      className="btn btn-ghost btn-icon"
      onClick={handleCopy}
      title={title || text}
      style={{ position: "relative", flexShrink: 0 }}
    >
      {copied ? (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M20 6L9 17l-5-5" />
        </svg>
      ) : (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      )}
    </button>
  );
}

export function Groups() {
  const { t } = useTranslation();
  const [details, setDetails] = useState<GroupDetail[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [groupStats, setGroupStats] = useState<Record<string, PlatformUsageStats>>({});
  const [loading, setLoading] = useState(true);

  // Edit mode
  const [editTarget, setEditTarget] = useState<GroupDetail | null>(null);
  const [editName, setEditName] = useState("");
  const [editPath, setEditPath] = useState("");
  const [editMode, setEditMode] = useState<RoutingMode>("failover");
  const [editPlatformIds, setEditPlatformIds] = useState<string[]>([]);
  const [editMappings, setEditMappings] = useState<{ id?: string; source_model: string; target_platform_id: string; target_model: string; request_timeout_secs?: number; connect_timeout_secs?: number }[]>([]);
  const [editReqTimeout, setEditReqTimeout] = useState(0);
  const [editConnTimeout, setEditConnTimeout] = useState(0);
  const [editSourceProtocol, setEditSourceProtocol] = useState("anthropic");

  // Create mode
  const [showCreate, setShowCreate] = useState(false);
  const [cName, setCName] = useState("");
  const [cPath, setCPath] = useState("/claude");
  const [cMode, setCMode] = useState<RoutingMode>("failover");

  // Mapping form (for quick add in list view)
  const [mappingGroupId, setMappingGroupId] = useState<string | null>(null);
  const [mSource, setMSource] = useState("");
  const [mTargetPlatform, setMTargetPlatform] = useState("");
  const [mTargetModel, setMTargetModel] = useState("");

  const load = async () => {
    setLoading(true);
    try {
      const [d, p] = await Promise.all([groupDetailApi.list(), platformApi.list()]);
      setDetails(d || []);
      setPlatforms(p || []);
      // Batch load group stats
      const statsMap: Record<string, PlatformUsageStats> = {};
      await Promise.all((d || []).map(async (g) => {
        try {
          const s = await groupUsageApi.stats(g.group.name);
          if (s && s.total_requests > 0) statsMap[g.group.name] = s;
        } catch { /* ignore */ }
      }));
      setGroupStats(statsMap);
    } catch (e) { console.error(e); }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  // ── Edit handlers ──

  const openEdit = (detail: GroupDetail) => {
    setEditTarget(detail);
    setEditName(detail.group.name);
    setEditPath(detail.group.path);
    setEditMode(detail.group.routing_mode);
    setEditPlatformIds(detail.platforms.map(gp => gp.platform.id));
    setEditMappings(detail.model_mappings.map(m => ({
      id: m.id,
      source_model: m.source_model,
      target_platform_id: m.target_platform_id,
      target_model: m.target_model,
      request_timeout_secs: m.request_timeout_secs,
      connect_timeout_secs: m.connect_timeout_secs,
    })));
    setEditReqTimeout(detail.group.request_timeout_secs);
      setEditSourceProtocol(detail.group.source_protocol || "anthropic");
    setEditConnTimeout(detail.group.connect_timeout_secs);
  };

  const cancelEdit = () => {
    setEditTarget(null);
    setEditName("");
    setEditPath("");
    setEditMode("failover");
    setEditPlatformIds([]);
    setEditMappings([]);
    setEditReqTimeout(0);
    setEditConnTimeout(0);
  };

  const saveEdit = async () => {
    if (!editTarget) return;
    try {
      // Update group basic info
      await groupApi.update({
        id: editTarget.group.id,
        name: editName,
        path: editPath,
        routing_mode: editMode,
        request_timeout_secs: editReqTimeout,
        connect_timeout_secs: editConnTimeout,
        source_protocol: editSourceProtocol,
      });

      // Update platforms
      await groupApi.setPlatforms(
        editTarget.group.id,
        editPlatformIds.map((pid, i) => ({ platform_id: pid, priority: i + 1, weight: 1 })),
      );

      // Diff mappings: delete old, create new
      const oldIds = new Set(editTarget.model_mappings.map(m => m.id));
      const keptIds = new Set(editMappings.filter(m => m.id).map(m => m.id!));
      for (const oldId of oldIds) {
        if (!keptIds.has(oldId)) {
          await mappingApi.delete(oldId);
        }
      }
      for (const m of editMappings) {
        if (m.id) {
          await mappingApi.update({
            id: m.id,
            source_model: m.source_model,
            target_platform_id: m.target_platform_id,
            target_model: m.target_model,
          });
        } else {
          await mappingApi.create({
            group_id: editTarget.group.id,
            source_model: m.source_model,
            target_platform_id: m.target_platform_id,
            target_model: m.target_model,
          });
        }
      }

      cancelEdit();
      load();
    } catch (e) {
      console.error(e);
      alert((e as any)?.toString?.() || "Failed to save group");
    }
  };

  // ── Create handler ──
  const handleCreateGroup = async () => {
    try {
      await groupApi.create({ name: cName, path: cPath, routing_mode: cMode });
      setCName(""); setCPath("/claude"); setCMode("failover"); setShowCreate(false);
      load();
    } catch (e) { console.error(e); }
  };

  const handleDeleteGroup = async (id: string) => {
    try {
      await groupApi.delete(id);
      load();
    } catch (e: any) {
      alert(e?.toString?.() || "Failed to delete group");
    }
  };

  // ── Quick mapping (list view) ──
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

  const selectedPlatform = platforms.find(p => p.id === mTargetPlatform);
  const availableModels = selectedPlatform ? allModelValues(selectedPlatform.models) : [];

  // ── Edit page ──
  if (editTarget) {
    const editPlatformOptions = platforms.filter(p => p.enabled);
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 720, width: "100%" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button className="btn btn-ghost btn-icon" onClick={cancelEdit} title={t("action.cancel")}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M19 12H5M12 19l-7-7 7-7" />
            </svg>
          </button>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: F.title, fontWeight: 700 }}>{editName || t("group.edit")}</div>
            <div className="text-secondary" style={{ fontSize: F.hint, marginTop: 2 }}>{editTarget.group.id.slice(0, 8)}</div>
          </div>
          <CopyButton text={buildClaudeCommand(editName)} title={t("group.copyCommand", "复制启动命令")} />
          <button className="btn" onClick={cancelEdit}>{t("action.cancel")}</button>
          <button className="btn btn-primary" onClick={saveEdit}
            disabled={!editName || !editPath}>{t("action.save")}</button>
        </div>

        {/* Basic info */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600, marginBottom: 4 }}>{t("group.basicInfo", "基本信息")}</div>

          {/* Name */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.name", "名称")}</span>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
              value={editName} onChange={e => setEditName(e.target.value)} />
          </div>

          {/* Path */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>Path</span>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
              value={editPath} onChange={e => setEditPath(e.target.value)} />
          </div>

          {/* Routing mode */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.routingMode", "路由模式")}</span>
            <select className="input" style={{ fontSize: F.body, padding: S.inputPad }}
              value={editMode} onChange={e => setEditMode(e.target.value as RoutingMode)}>
              <option value="failover">{t("group.failover")}</option>
              <option value="load_balance">{t("group.loadBalance")}</option>
            </select>
          </div>

          {/* Timeout */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.timeout", "超时")}</span>
            <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
              <input className="input" type="number" min={0} placeholder={t("group.reqTimeout", "请求(s)")}
                value={editReqTimeout || ""} onChange={e => setEditReqTimeout(Math.max(0, Number(e.target.value)))}
                style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
              <input className="input" type="number" min={0} placeholder={t("group.connTimeout", "连接(s)")}
                value={editConnTimeout || ""} onChange={e => setEditConnTimeout(Math.max(0, Number(e.target.value)))}
                style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>0 = 系统默认</span>
            </div>
          </div>

          {/* Source protocol */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.sourceProtocol", "入站协议")}</span>
            <select className="input" value={editSourceProtocol} onChange={e => setEditSourceProtocol(e.target.value)}
              style={{ fontSize: F.body, padding: S.inputPad, width: 160 }}>
              <option value="anthropic">Anthropic</option>
              <option value="openai">OpenAI</option>
            </select>
          </div>

          {/* Auto badge */}
          {editTarget.group.auto_from_platform && (
            <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: F.hint, color: "var(--text-tertiary)" }}>
              <span className="badge badge-muted" style={{ fontSize: 10, padding: "0 5px" }}>auto</span>
              {t("group.autoFromPlatform", "自动创建，部分字段不可编辑")}
            </div>
          )}
        </div>

        {/* Platforms */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("group.platforms", "关联平台")}</div>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
            {t("group.platformsHint", "选择并排序此分组使用的平台，顺序决定优先级")}
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {/* Selected platforms — reorderable by drag later, for now ordered list */}
            {editPlatformIds.map((pid, i) => {
              const p = platforms.find(pp => pp.id === pid);
              if (!p) return null;
              return (
                <div key={pid} style={{
                  display: "flex", alignItems: "center", gap: 10,
                  padding: "8px 12px", borderRadius: "var(--radius-sm)",
                  background: "var(--bg-glass)", border: "1px solid var(--border)",
                }}>
                  <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", width: 20, textAlign: "center" }}>
                    {i + 1}
                  </span>
                  <span style={{
                    width: 28, height: 28, borderRadius: "var(--radius-sm)",
                    display: "flex", alignItems: "center", justifyContent: "center",
                    background: "var(--accent-subtle)", color: "var(--accent)",
                    fontSize: 11, fontWeight: 700, flexShrink: 0,
                  }}>
                    {p.protocol.slice(0, 2).toUpperCase()}
                  </span>
                  <span style={{ flex: 1, fontSize: F.body, fontWeight: 500 }}>{p.name}</span>
                  {/* Move up/down */}
                  <button type="button" className="btn btn-ghost btn-icon" style={{ width: 24, height: 24, minWidth: 24, padding: 0 }}
                    disabled={i === 0}
                    onClick={() => {
                      const ids = [...editPlatformIds];
                      [ids[i - 1], ids[i]] = [ids[i], ids[i - 1]];
                      setEditPlatformIds(ids);
                    }}>
                    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                      <path d="M5 2v6M2 5l3-3 3 3" />
                    </svg>
                  </button>
                  <button type="button" className="btn btn-ghost btn-icon" style={{ width: 24, height: 24, minWidth: 24, padding: 0 }}
                    disabled={i === editPlatformIds.length - 1}
                    onClick={() => {
                      const ids = [...editPlatformIds];
                      [ids[i], ids[i + 1]] = [ids[i + 1], ids[i]];
                      setEditPlatformIds(ids);
                    }}>
                    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                      <path d="M5 8V2M2 5l3 3 3-3" />
                    </svg>
                  </button>
                  <button type="button" onClick={() => setEditPlatformIds(editPlatformIds.filter(id => id !== pid))} style={{
                    background: "none", border: "none", cursor: "pointer",
                    color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1,
                  }}>✕</button>
                </div>
              );
            })}
          </div>
          {/* Add platform */}
          {editPlatformIds.length < editPlatformOptions.length && (
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <select className="input" style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                onChange={e => {
                  if (e.target.value && !editPlatformIds.includes(e.target.value)) {
                    setEditPlatformIds([...editPlatformIds, e.target.value]);
                  }
                  e.target.value = "";
                }}>
                <option value="">{t("group.addPlatform", "+ 添加平台")}</option>
                {editPlatformOptions
                  .filter(p => !editPlatformIds.includes(p.id))
                  .map(p => <option key={p.id} value={p.id}>{p.name} ({p.protocol})</option>)}
              </select>
            </div>
          )}
        </div>

        {/* Model Mappings */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("group.modelMappings", "模型映射")}</div>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
            {t("group.mappingsHint", "将源模型名映射到目标平台的具体模型")}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {editMappings.map((m, i) => {
              const targetPlat = platforms.find(p => p.id === m.target_platform_id);
              const models = targetPlat ? allModelValues(targetPlat.models) : [];
              return (
                <div key={i} style={{
                  display: "flex", gap: 8, alignItems: "center",
                  padding: "8px 12px", borderRadius: "var(--radius-sm)",
                  background: "var(--bg-glass)", border: "1px solid var(--border)",
                }}>
                  <input className="input" style={{ fontSize: F.hint, padding: "6px 10px", width: 140, flexShrink: 0 }}
                    placeholder={t("mapping.source", "源模型")}
                    value={m.source_model}
                    onChange={e => {
                      const ms = [...editMappings];
                      ms[i] = { ...ms[i], source_model: e.target.value };
                      setEditMappings(ms);
                    }} />
                  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round">
                    <path d="M2 6h8M8 4l2 2-2 2" />
                  </svg>
                  <select className="input" style={{ fontSize: F.hint, padding: "6px 10px", width: 140, flexShrink: 0 }}
                    value={m.target_platform_id}
                    onChange={e => {
                      const ms = [...editMappings];
                      ms[i] = { ...ms[i], target_platform_id: e.target.value, target_model: "" };
                      setEditMappings(ms);
                    }}>
                    <option value="">{t("mapping.targetPlatform", "目标平台")}</option>
                    {platforms.filter(p => p.enabled).map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                  {models.length > 0 ? (
                    <select className="input" style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                      value={m.target_model}
                      onChange={e => {
                        const ms = [...editMappings];
                        ms[i] = { ...ms[i], target_model: e.target.value };
                        setEditMappings(ms);
                      }}>
                      <option value="">{t("mapping.target", "目标模型")}</option>
                      {models.map(m2 => <option key={m2} value={m2}>{m2}</option>)}
                    </select>
                  ) : (
                    <input className="input" style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                      placeholder={t("mapping.target", "目标模型")}
                      value={m.target_model}
                      onChange={e => {
                        const ms = [...editMappings];
                        ms[i] = { ...ms[i], target_model: e.target.value };
                        setEditMappings(ms);
                      }} />
                  )}
                  <button type="button" onClick={() => setEditMappings(editMappings.filter((_, j) => j !== i))} style={{
                    background: "none", border: "none", cursor: "pointer",
                    color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1, flexShrink: 0,
                  }}>✕</button>
                </div>
              );
            })}

            <button type="button" className="btn btn-ghost" style={{ fontSize: F.hint, padding: "6px 12px", alignSelf: "flex-start" }}
              onClick={() => setEditMappings([...editMappings, { source_model: "", target_platform_id: "", target_model: "" }])}>
              + {t("mapping.add", "添加映射")}
            </button>
          </div>
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
          <div className="section-title">{t("page.groups")}</div>
          <div className="section-desc">
            {details.length > 0 ? `${details.length} ${t("nav.groups").toLowerCase()}` : t("group.empty")}
          </div>
        </div>
        <button className="btn btn-primary" onClick={() => setShowCreate(true)}>
          + {t("group.add")}
        </button>
      </div>

      {/* Create Group Form */}
      {showCreate && (
        <div className="glass-surface animate-fade-in" style={{
          padding: 20,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}>
          <input className="input" placeholder={t("group.name")} value={cName}
            onChange={(e) => setCName(e.target.value)} />
          <div style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: -4 }}>
            仅允许小写字母、数字和连字符（自动转换）
          </div>
          <input className="input" placeholder="Path (e.g. /claude)" value={cPath}
            onChange={(e) => setCPath(e.target.value)} />
          <select className="input" value={cMode} onChange={(e) => setCMode(e.target.value as RoutingMode)}>
            <option value="failover">{t("group.failover")}</option>
            <option value="load_balance">{t("group.loadBalance")}</option>
          </select>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button className="btn" onClick={() => setShowCreate(false)}>{t("action.cancel")}</button>
            <button className="btn btn-primary" onClick={handleCreateGroup}
              disabled={!cName || !cPath}>{t("action.create")}</button>
          </div>
        </div>
      )}

      {/* Group List */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {details.length === 0 && !showCreate && (
            <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
              <div className="text-tertiary" style={{ fontSize: 13 }}>{t("group.empty")}</div>
            </div>
          )}
          {details.map(({ group, platforms: gps, model_mappings }, i) => (
            <div
              key={group.id}
              className="card-item animate-fade-in"
              style={{ animationDelay: `${i * 60}ms`, cursor: "pointer" }}
              onClick={() => openEdit({ group, platforms: gps, model_mappings })}
            >
              {/* Group Header */}
              <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 10 }}>
                <div style={{
                  width: 32, height: 32, borderRadius: "var(--radius-sm)",
                  display: "flex", alignItems: "center", justifyContent: "center",
                  background: group.auto_from_platform ? "var(--bg-glass)" : "var(--accent-subtle)",
                  color: group.auto_from_platform ? "var(--text-secondary)" : "var(--accent)",
                  fontSize: 13, fontWeight: 700,
                  flexShrink: 0,
                }}>
                  {group.path.slice(0, 3)}
                </div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 600, fontSize: 14, display: "flex", alignItems: "center", gap: 6 }}>
                    {group.name}
                    {group.auto_from_platform && (
                      <span className="badge badge-muted" style={{ fontSize: 10, padding: "0 5px", fontWeight: 500 }}>auto</span>
                    )}
                  </div>
                  <div className="text-secondary" style={{ fontSize: 12, display: "flex", gap: 8, marginTop: 1 }}>
                    <span>{group.path}</span>
                    <span className="badge badge-muted" style={{ padding: "0 6px" }}>
                      {group.routing_mode === "failover" ? t("group.failover") : t("group.loadBalance")}
                    </span>
                  </div>
                </div>
                {/* Inline token stats in header */}
                {groupStats[group.name] && (() => {
                  const u = groupStats[group.name];
                  const total = u.total_input_tokens + u.total_output_tokens;
                  if (total === 0 && u.total_requests === 0) return null;
                  return (
                    <div style={{ display: "flex", gap: 4, alignItems: "center", marginRight: 4 }}>
                      <span style={{ fontSize: 11, fontWeight: 700, color: "var(--accent)" }}>{fmtTk(total)}</span>
                      <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>tokens</span>
                      {u.total_requests > 0 && (
                        <>
                          <span style={{ fontSize: 10, color: "var(--text-tertiary)", marginLeft: 4 }}>·</span>
                          <span style={{ fontSize: 11, fontWeight: 600, color: u.success_count / u.total_requests >= 0.95 ? "var(--color-success, #34c759)" : "var(--color-warning, #ff9500)" }}>
                            {(u.success_count / u.total_requests * 100).toFixed(0)}%
                          </span>
                        </>
                      )}
                    </div>
                  );
                })()}
                <CopyButton text={buildClaudeCommand(group.name)} title={t("group.copyCommand", "复制启动命令")} size={14} />
                <button className="btn btn-ghost btn-icon" onClick={e => { e.stopPropagation(); openEdit({ group, platforms: gps, model_mappings }); }} title={t("action.edit", "编辑")}>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                  </svg>
                </button>
                {!group.auto_from_platform && (
                  <button className="btn btn-ghost btn-icon btn-danger" onClick={(e) => { e.stopPropagation(); handleDeleteGroup(group.id); }}>
                    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                    </svg>
                  </button>
                )}
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

              {/* Usage Stats */}
              {groupStats[group.name] && (() => {
                const u = groupStats[group.name];
                const total = u.total_input_tokens + u.total_output_tokens;
                const cost = estCost(u.total_input_tokens, u.total_output_tokens);
                const successRate = u.total_requests > 0 ? (u.success_count / u.total_requests * 100) : 0;
                return (
                  <div style={{ display: "flex", gap: 6, marginBottom: 8 }}>
                    <StatChip icon="⚡" value={fmtTk(total)} label="tokens" />
                    <StatChip icon="💰" value={`$${cost}`} label="cost" />
                    <StatChip icon="📦" value={`${u.cache_rate.toFixed(1)}%`} label="cache" color="var(--color-success, #34c759)" />
                    <StatChip icon="✓" value={`${successRate.toFixed(1)}%`} label="ok"
                      color={successRate >= 95 ? "var(--color-success, #34c759)" : successRate >= 80 ? "var(--color-warning, #ff9500)" : "var(--color-danger, #ff3b30)"} />
                  </div>
                );
              })()}

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
                        onClick={(e) => { e.stopPropagation(); handleDeleteMapping(m.id); }}>
                        <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round">
                          <path d="M2 2l6 6M8 2l-6 6" />
                        </svg>
                      </button>
                    </div>
                  ))}
                </div>
              )}

              {/* Quick Add Mapping */}
              <button className="btn btn-ghost" style={{ fontSize: 12, gap: 4, padding: "4px 8px", color: "var(--text-secondary)" }}
                onClick={(e) => { e.stopPropagation(); setMappingGroupId(group.id); }}>
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
                }} onClick={e => e.stopPropagation()}>
                  <input className="input" style={{ flex: 1, minWidth: 100, fontSize: 12 }}
                    placeholder={t("mapping.source")} value={mSource}
                    onChange={(e) => setMSource(e.target.value)} />
                  <select className="input" style={{ fontSize: 12, width: 140 }} value={mTargetPlatform}
                    onChange={(e) => { setMTargetPlatform(e.target.value); setMTargetModel(""); }}>
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

function fmtTk(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return `${n}`;
}

function estCost(inputTokens: number, outputTokens: number): string {
  const cost = (inputTokens / 1_000_000) * 3 + (outputTokens / 1_000_000) * 12;
  if (cost >= 1) return cost.toFixed(2);
  if (cost >= 0.01) return cost.toFixed(3);
  if (cost > 0) return cost.toFixed(4);
  return "0";
}

function StatChip({ icon, value, label, color }: { icon: string; value: string; label: string; color?: string }) {
  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 5,
      padding: "4px 10px", borderRadius: "var(--radius-sm)",
      background: "var(--bg-glass)", border: "1px solid var(--border)",
      fontSize: 12,
    }}>
      <span style={{ fontSize: 13 }}>{icon}</span>
      <span style={{ fontWeight: 700, color: color || "var(--text-primary)" }}>{value}</span>
      <span style={{ fontSize: 10, color: "var(--text-tertiary)", fontWeight: 500 }}>{label}</span>
    </div>
  );
}
