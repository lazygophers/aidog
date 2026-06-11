import { useState, useEffect, useRef, Fragment } from "react";
import { useTranslation } from "react-i18next";
import {
  groupDetailApi, groupApi, platformApi,
  type GroupDetail, type Platform, type RoutingMode, type ModelSlot, type PlatformUsageStats,
  type ModelMapping,
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
  const [_platformStats, setPlatformStats] = useState<Record<string, PlatformUsageStats>>({});
  const [loading, setLoading] = useState(true);

  // Edit mode
  const [editTarget, setEditTarget] = useState<GroupDetail | null>(null);
  const [editName, setEditName] = useState("");
  const [editPath, setEditPath] = useState("");
  const [editMode, setEditMode] = useState<RoutingMode>("failover");
  const [editPlatformIds, setEditPlatformIds] = useState<number[]>([]);
  const [editMappings, setEditMappings] = useState<ModelMapping[]>([]);
  const [editReqTimeout, setEditReqTimeout] = useState(0);
  const [editConnTimeout, setEditConnTimeout] = useState(0);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);

  // ── Drag reorder for group list ──
  const [groupDrag, setGroupDrag] = useState<{ from: number; to: number } | null>(null);
  const groupListRef = useRef<HTMLDivElement>(null);
  const groupDragStartRef = useRef<{ y: number; index: number } | null>(null);
  const groupDidDragRef = useRef(false);

  const handleGroupPointerDown = (e: React.PointerEvent, index: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    groupDragStartRef.current = { y: e.clientY, index };
  };

  const handleGroupPointerMove = (e: React.PointerEvent) => {
    const start = groupDragStartRef.current;
    if (!start) return;
    if (!groupDrag) {
      if (Math.abs(e.clientY - start.y) < 5) return;
      setGroupDrag({ from: start.index, to: start.index });
      groupDidDragRef.current = true;
    }
    if (!groupListRef.current) return;
    const cards = groupListRef.current.querySelectorAll<HTMLElement>("[data-group-id]");
    let newTo = cards.length;
    for (let i = 0; i < cards.length; i++) {
      const rect = cards[i].getBoundingClientRect();
      if (e.clientY < rect.top + rect.height / 2) { newTo = i; break; }
    }
    setGroupDrag(d => d ? { ...d, to: newTo } : null);
  };

  const handleGroupPointerUp = () => {
    if (groupDrag) {
      const effectiveTo = groupDrag.from < groupDrag.to ? groupDrag.to - 1 : groupDrag.to;
      if (groupDrag.from !== effectiveTo) {
        const reordered = [...details];
        const [moved] = reordered.splice(groupDrag.from, 1);
        reordered.splice(effectiveTo, 0, moved);
        setDetails(reordered);
        groupApi.reorder(reordered.map(d => d.group.id)).catch(console.error);
      }
    }
    setGroupDrag(null);
    groupDragStartRef.current = null;
    setTimeout(() => { groupDidDragRef.current = false; }, 50);
  };

  // ── Drag reorder for selected platforms ──
  const reorderPlatforms = (from: number, to: number) => {
    if (from === to) return;
    const ids = [...editPlatformIds];
    const [moved] = ids.splice(from, 1);
    ids.splice(to, 0, moved);
    setEditPlatformIds(ids);
  };

  // Create mode
  const [showCreate, setShowCreate] = useState(false);
  const [cName, setCName] = useState("");
  const [cPath, setCPath] = useState("/claude");
  const [cMode, setCMode] = useState<RoutingMode>("failover");

  // Mapping form (for quick add in list view)
  const [mappingGroupId, setMappingGroupId] = useState<number | null>(null);
  const [mSource, setMSource] = useState("");
  const [mTargetPlatform, setMTargetPlatform] = useState<number | "">("");
  const [mTargetModel, setMTargetModel] = useState("");

  const load = async () => {
    setLoading(true);
    try {
      const [d, p] = await Promise.all([groupDetailApi.list(), platformApi.list()]);
      setDetails(d || []);
      setPlatforms(p || []);
      // Load per-platform usage stats
      const pStatsMap: Record<string, PlatformUsageStats> = {};
      await Promise.all((p || []).map(async (plat) => {
        try {
          const s = await platformApi.usageStats(plat.id);
          if (s && s.total_requests > 0) pStatsMap[plat.id] = s;
        } catch { /* ignore */ }
      }));
      setPlatformStats(pStatsMap);
      // Aggregate group stats from associated platform stats
      const statsMap: Record<string, PlatformUsageStats> = {};
      for (const g of d || []) {
        let total_requests = 0, success_count = 0;
        let total_input_tokens = 0, total_output_tokens = 0, total_cache_tokens = 0;
        for (const gp of g.platforms) {
          const ps = pStatsMap[gp.platform.id];
          if (ps) {
            total_requests += ps.total_requests;
            success_count += ps.success_count;
            total_input_tokens += ps.total_input_tokens;
            total_output_tokens += ps.total_output_tokens;
            total_cache_tokens += ps.total_cache_tokens;
          }
        }
        if (total_requests > 0) {
          statsMap[g.group.name] = {
            total_requests, success_count,
            total_input_tokens, total_output_tokens, total_cache_tokens,
            cache_rate: total_input_tokens > 0 ? total_cache_tokens / total_input_tokens * 100 : 0,
            recent_failures: 0, recent_total: 0,
          };
        }
      }
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
      source_model: m.source_model,
      target_platform_id: m.target_platform_id,
      target_model: m.target_model,
      request_timeout_secs: m.request_timeout_secs,
      connect_timeout_secs: m.connect_timeout_secs,
    })));
    setEditReqTimeout(detail.group.request_timeout_secs);
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
      // Update group basic info + inline model mappings
      await groupApi.update({
        id: editTarget.group.id,
        name: editName,
        path: editPath,
        routing_mode: editMode,
        request_timeout_secs: editReqTimeout,
        connect_timeout_secs: editConnTimeout,
        model_mappings: editMappings,
      });

      // Update platforms
      await groupApi.setPlatforms(
        editTarget.group.id,
        editPlatformIds.map((pid, i) => ({ platform_id: pid, priority: i + 1, weight: 1 })),
      );

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

  const handleDeleteGroup = async (id: number) => {
    try {
      await groupApi.delete(id);
      load();
    } catch (e: any) {
      alert(e?.toString?.() || "Failed to delete group");
    }
  };

  // ── Quick mapping (list view) — persists inline via group.update ──
  const handleAddMapping = async () => {
    if (!mappingGroupId || !mSource || mTargetPlatform === "" || !mTargetModel) return;
    const detail = details.find(d => d.group.id === mappingGroupId);
    if (!detail) return;
    try {
      const next: ModelMapping[] = [
        ...detail.model_mappings,
        {
          source_model: mSource,
          target_platform_id: mTargetPlatform,
          target_model: mTargetModel,
          request_timeout_secs: 0,
          connect_timeout_secs: 0,
        },
      ];
      await groupApi.update({ id: mappingGroupId, model_mappings: next });
      setMSource(""); setMTargetPlatform(""); setMTargetModel("");
      setMappingGroupId(null);
      load();
    } catch (e) { console.error(e); }
  };

  const handleDeleteMapping = async (groupId: number, index: number) => {
    const detail = details.find(d => d.group.id === groupId);
    if (!detail) return;
    try {
      const next = detail.model_mappings.filter((_, i) => i !== index);
      await groupApi.update({ id: groupId, model_mappings: next });
      load();
    } catch (e) { console.error(e); }
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
            <div className="text-secondary" style={{ fontSize: F.hint, marginTop: 2 }}>#{editTarget.group.id}</div>
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
            {/* Selected platforms — drag to reorder (order = routing priority) */}
            {editPlatformIds.map((pid, i) => {
              const p = platforms.find(pp => pp.id === pid);
              if (!p) return null;
              const isDragging = dragIndex === i;
              const isDragOver = dragOverIndex === i && dragIndex !== null && dragIndex !== i;
              return (
                <div key={pid}
                  draggable
                  onDragStart={e => { setDragIndex(i); e.dataTransfer.effectAllowed = "move"; }}
                  onDragOver={e => { e.preventDefault(); e.dataTransfer.dropEffect = "move"; if (dragOverIndex !== i) setDragOverIndex(i); }}
                  onDragLeave={() => { if (dragOverIndex === i) setDragOverIndex(null); }}
                  onDrop={e => { e.preventDefault(); if (dragIndex !== null) reorderPlatforms(dragIndex, i); setDragIndex(null); setDragOverIndex(null); }}
                  onDragEnd={() => { setDragIndex(null); setDragOverIndex(null); }}
                  style={{
                  display: "flex", alignItems: "center", gap: 10,
                  padding: "8px 12px", borderRadius: "var(--radius-sm)",
                  background: "var(--bg-glass)",
                  border: isDragOver ? "1px solid var(--accent)" : "1px solid var(--border)",
                  opacity: isDragging ? 0.4 : 1,
                  boxShadow: isDragOver ? "0 0 0 1px var(--accent) inset" : undefined,
                  transition: "opacity 0.15s, border-color 0.15s",
                }}>
                  <span title={t("group.dragToReorder", "拖动排序")} style={{
                    cursor: "grab", color: "var(--text-tertiary)", fontSize: 14,
                    lineHeight: 1, userSelect: "none", flexShrink: 0,
                  }}>⠿</span>
                  <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", width: 20, textAlign: "center" }}>
                    {i + 1}
                  </span>
                  <span style={{
                    width: 28, height: 28, borderRadius: "var(--radius-sm)",
                    display: "flex", alignItems: "center", justifyContent: "center",
                    background: "var(--accent-subtle)", color: "var(--accent)",
                    fontSize: 11, fontWeight: 700, flexShrink: 0,
                  }}>
                    {p.platform_type.slice(0, 2).toUpperCase()}
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
                  const pid = Number(e.target.value);
                  if (e.target.value && !editPlatformIds.includes(pid)) {
                    setEditPlatformIds([...editPlatformIds, pid]);
                  }
                  e.target.value = "";
                }}>
                <option value="">{t("group.addPlatform", "+ 添加平台")}</option>
                {editPlatformOptions
                  .filter(p => !editPlatformIds.includes(p.id))
                  .map(p => <option key={p.id} value={p.id}>{p.name} ({p.platform_type})</option>)}
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
                    value={m.target_platform_id || ""}
                    onChange={e => {
                      const ms = [...editMappings];
                      ms[i] = { ...ms[i], target_platform_id: e.target.value === "" ? 0 : Number(e.target.value), target_model: "" };
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
              onClick={() => setEditMappings([...editMappings, { source_model: "", target_platform_id: 0, target_model: "", request_timeout_secs: 0, connect_timeout_secs: 0 }])}>
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
        <div ref={groupListRef} style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {details.length === 0 && !showCreate && (
            <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
              <div className="text-tertiary" style={{ fontSize: 13 }}>{t("group.empty")}</div>
            </div>
          )}
          {details.map(({ group, platforms: gps, model_mappings }, i) => {
            const isDragging = groupDrag?.from === i;
            return (
            <Fragment key={group.id}>
              {groupDrag && groupDrag.to === i && (() => {
                const dg = details[groupDrag.from];
                const routeLabel = dg.group.routing_mode === "failover" ? t("group.failover") : t("group.loadBalance");
                return (
                  <div style={{
                    display: "flex", alignItems: "center", gap: 10, paddingLeft: 44,
                    padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                    background: "var(--glass-bg, rgba(255,255,255,0.06))",
                    border: "1.5px dashed var(--accent)",
                    opacity: 0.5, filter: "grayscale(0.8)",
                    pointerEvents: "none", transition: "all 150ms ease",
                  }}>
                    <div style={{
                      width: 24, height: 24, borderRadius: "var(--radius-sm)",
                      display: "flex", alignItems: "center", justifyContent: "center",
                      background: "var(--bg-glass)", fontSize: 11, fontWeight: 700,
                      color: "var(--text-secondary)", flexShrink: 0,
                    }}>
                      {dg.group.path.slice(0, 3)}
                    </div>
                    <span style={{ fontSize: 13, fontWeight: 600 }}>{dg.group.name}</span>
                    <span className="badge badge-muted" style={{ fontSize: 10, padding: "0 6px" }}>{routeLabel}</span>
                  </div>
                );
              })()}
            <div
              data-group-id={group.id}
              className={`card-item animate-fade-in${isDragging ? " is-dragging" : ""}`}
              style={{
                position: "relative",
                paddingLeft: 44,
                animationDelay: `${i * 60}ms`,
                cursor: "pointer",
                opacity: groupDrag ? (isDragging ? 0 : 0.4) : undefined,
                ...(isDragging ? { height: 0, overflow: "hidden", padding: 0, margin: 0, borderWidth: 0, minHeight: 0 } : {}),
                transition: "transform 200ms ease, box-shadow 200ms ease, opacity 150ms ease",
              }}
              onClick={() => {
                if (groupDidDragRef.current) return;
                openEdit({ group, platforms: gps, model_mappings });
              }}
            >
              <div
                className={`drag-handle${isDragging ? " is-active" : ""}`}
                onPointerDown={e => handleGroupPointerDown(e, i)}
                onPointerMove={handleGroupPointerMove}
                onPointerUp={handleGroupPointerUp}
              >
                <svg width="14" height="20" viewBox="0 0 14 20" fill="currentColor"><circle cx="4" cy="3" r="1.8"/><circle cx="4" cy="10" r="1.8"/><circle cx="4" cy="17" r="1.8"/><circle cx="10" cy="3" r="1.8"/><circle cx="10" cy="10" r="1.8"/><circle cx="10" cy="17" r="1.8"/></svg>
              </div>
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
                  const cost = estCost(u.total_input_tokens, u.total_output_tokens);
                  return (
                    <div style={{ display: "flex", gap: 4, alignItems: "center", marginRight: 4 }}>
                      <span style={{ fontSize: 11, fontWeight: 700, color: "var(--accent)" }}>{fmtTk(total)}</span>
                      <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>tokens</span>
                      {Number(cost) > 0 && (
                        <>
                          <span style={{ fontSize: 10, color: "var(--text-tertiary)", marginLeft: 4 }}>·</span>
                          <span style={{ fontSize: 11, fontWeight: 600, color: "var(--color-success, #34c759)" }}>${cost}</span>
                        </>
                      )}
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
                  {model_mappings.map((m, mi) => (
                    <div key={mi} style={{
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
                        onClick={(e) => { e.stopPropagation(); handleDeleteMapping(group.id, mi); }}>
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
                    onChange={(e) => { setMTargetPlatform(e.target.value === "" ? "" : Number(e.target.value)); setMTargetModel(""); }}>
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
            </Fragment>
          );
          })}
          {groupDrag && (() => {
            if (groupDrag.to !== details.length) return null;
            const dg = details[groupDrag.from];
            const routeLabel = dg.group.routing_mode === "failover" ? t("group.failover") : t("group.loadBalance");
            return (
              <div style={{
                display: "flex", alignItems: "center", gap: 10, paddingLeft: 44,
                padding: "10px 16px", margin: "2px 0", borderRadius: 12,
                background: "var(--glass-bg, rgba(255,255,255,0.06))",
                border: "1.5px dashed var(--accent)",
                opacity: 0.5, filter: "grayscale(0.8)",
                pointerEvents: "none", transition: "all 150ms ease",
              }}>
                <div style={{
                  width: 24, height: 24, borderRadius: "var(--radius-sm)",
                  display: "flex", alignItems: "center", justifyContent: "center",
                  background: "var(--bg-glass)", fontSize: 11, fontWeight: 700,
                  color: "var(--text-secondary)", flexShrink: 0,
                }}>
                  {dg.group.path.slice(0, 3)}
                </div>
                <span style={{ fontSize: 13, fontWeight: 600 }}>{dg.group.name}</span>
                <span className="badge badge-muted" style={{ fontSize: 10, padding: "0 6px" }}>{routeLabel}</span>
              </div>
            );
          })()}
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
