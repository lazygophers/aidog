import { useState, useEffect, useReducer, useMemo, Fragment } from "react";
import type { DragEvent as ReactDragEvent, ReactNode } from "react";
import { useTranslation } from "react-i18next";
import claudeIcon from "../assets/platforms/claude_code.svg";
import codexIcon from "../assets/platforms/openai.svg";
import type { TFunction } from "i18next";
import {
  groupDetailApi, groupApi, groupUsageApi, platformApi, proxyApi, onProxyLogUpdated,
  type GroupDetail, type GroupPlatformDetail, type Platform, type RoutingMode, type ModelSlot, type PlatformUsageStats,
  type ModelMapping,
} from "../services/api";
import { SortableList } from "../components/SortableList";
import { IconClose, IconCheck, IconBolt, IconCost } from "../components/icons";
import { formatNumber, formatCost, formatPercent, successRate as calcSuccessRate } from "../utils/formatters";
import { CompactCard, StatChip, BalanceBar, successRateLevel, costLevel } from "../components/shared";
import { getPlatformLogo, getFaviconUrl } from "../assets/platforms";
import { MiddlewareRulesPanel } from "../components/settings/MiddlewareRules";
import { PlatformCard, type PlatformCardActions } from "../components/platforms/PlatformCard";
import { usePlatformCards, computeQuotaDisplay } from "../components/platforms/usePlatformCards";

const MODEL_SLOTS: ModelSlot[] = ["default", "sonnet", "opus", "haiku", "gpt"];

/** 全部调度策略（与 api.ts RoutingMode 契约对齐，禁裸 string）。 */
const ROUTING_MODES: RoutingMode[] = ["failover", "load_balance", "health_aware", "least_latency", "sticky"];

/** 策略短名（i18n，缺键回退默认中文）。 */
function routingModeLabel(t: TFunction, mode: RoutingMode): string {
  const map: Record<RoutingMode, string> = {
    failover: t("group.failover", "故障转移"),
    load_balance: t("group.loadBalance", "负载均衡"),
    health_aware: t("group.routingMode.health_aware", "健康感知"),
    least_latency: t("group.routingMode.least_latency", "最低延迟"),
    sticky: t("group.routingMode.sticky", "会话粘性"),
  };
  return map[mode] ?? mode;
}

/** 策略说明（下拉旁提示）。 */
function routingModeDesc(t: TFunction, mode: RoutingMode): string {
  const map: Record<RoutingMode, string> = {
    failover: t("group.routingModeDesc.failover", "按优先级升序选平台，失败逐个回退。"),
    load_balance: t("group.routingModeDesc.load_balance", "在可用平台间加权随机分流。"),
    health_aware: t("group.routingModeDesc.health_aware", "摘除熔断平台后，在健康平台间加权随机。"),
    least_latency: t("group.routingModeDesc.least_latency", "按各平台延迟均值升序优先选最快平台。"),
    sticky: t("group.routingModeDesc.sticky", "同会话绑定同一平台，失效/熔断后回退加权随机。"),
  };
  return map[mode] ?? "";
}

/** Group 图标：仅关联 1 个平台时跟随该平台 logo（与 Platforms 页一致），否则回退分组名首字文字框。 */
function GroupIcon({ gps, group }: { gps: GroupDetail["platforms"]; group: GroupDetail["group"] }) {
  const [favFailed, setFavFailed] = useState(false);
  const single = gps.length === 1 ? gps[0].platform : null;
  const logo = single ? getPlatformLogo(single.platform_type) : undefined;
  const favicon = single && !logo && !favFailed ? getFaviconUrl(single) : null;
  const box = {
    width: 32, height: 32, borderRadius: "var(--radius-sm)", flexShrink: 0,
    display: "flex", alignItems: "center", justifyContent: "center",
  } as const;
  if (single && (logo || favicon)) {
    return (
      <div style={{ ...box, background: "transparent" }}>
        <img src={(logo || favicon) as string} alt={single.name}
          onError={() => { if (favicon) setFavFailed(true); }}
          style={{ width: "100%", height: "100%", objectFit: "contain", padding: 4 }} />
      </div>
    );
  }
  return (
    <div style={{
      ...box,
      background: group.auto_from_platform ? "var(--bg-glass)" : "var(--accent-subtle)",
      color: group.auto_from_platform ? "var(--text-secondary)" : "var(--accent)",
      fontSize: 13, fontWeight: 700,
    }}>
      {group.name.slice(0, 3)}
    </div>
  );
}

/** Row model for the sortable selected-platforms list (stable string id for @dnd-kit). */
interface SortablePlatform {
  id: string;
  platformId: number;
}

/** Row model for the sortable group list (GroupDetail has no top-level stable id). */
interface GroupRow {
  id: string;
  detail: GroupDetail;
}

/** 分组编辑表单态（原 8 个 useState 合并为单 reducer，减少分散 setState） */
interface EditState {
  target: GroupDetail | null;
  name: string;
  mode: RoutingMode;
  platformIds: number[];
  mappings: ModelMapping[];
  reqTimeout: number;
  connTimeout: number;
  maxRetries: number;
}

const EMPTY_EDIT: EditState = {
  target: null,
  name: "",
  mode: "failover",
  platformIds: [],
  mappings: [],
  reqTimeout: 0,
  connTimeout: 0,
  maxRetries: 10,
};

type EditAction =
  | { type: "open"; detail: GroupDetail }
  | { type: "reset" }
  | { type: "patch"; patch: Partial<EditState> };

function editReducer(state: EditState, action: EditAction): EditState {
  switch (action.type) {
    case "open":
      return {
        target: action.detail,
        name: action.detail.group.name,
        mode: action.detail.group.routing_mode,
        platformIds: action.detail.platforms.map(gp => gp.platform.id),
        mappings: action.detail.model_mappings.map(m => ({
          source_model: m.source_model,
          target_platform_id: m.target_platform_id,
          target_model: m.target_model,
          request_timeout_secs: m.request_timeout_secs,
          connect_timeout_secs: m.connect_timeout_secs,
        })),
        reqTimeout: action.detail.group.request_timeout_secs,
        connTimeout: action.detail.group.connect_timeout_secs,
        maxRetries: action.detail.group.max_retries,
      };
    case "reset":
      return EMPTY_EDIT;
    case "patch":
      return { ...state, ...action.patch };
  }
}

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

/** POSIX shell 单引号安全转义（内部单引号闭合/转义/重开），杜绝注入。 */
function shellSquote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

/**
 * Build the `codex` CLI invocation for a given group profile.
 * `AIDOG_KEY=<group>`（auth token=分组名，aidog 据此路由）+ `codex -p <group>`
 * 选 `~/.codex/<group>.config.toml` profile + bypass approvals/sandbox。
 */
function buildCodexCommand(groupKey: string): string {
  const g = shellSquote(groupKey);
  return [
    `AIDOG_KEY=${g}`,
    "codex",
    "-p",
    g,
    "--dangerously-bypass-approvals-and-sandbox",
    "-a",
    "never",
  ].join(" ");
}

// ─── Design tokens ───
const F = { title: 20, label: 15, body: 15, hint: 13, small: 12 } as const;
const S = { gap: 18, pad: 28, inputPad: "10px 14px", btnPad: "8px 18px", btnIcon: 34 } as const;

/** Copy text to clipboard with a brief visual feedback */
function CopyButton({ text, title, label, icon, size = 14 }: { text: string; title?: string; label?: string; icon?: ReactNode; size?: number }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  const hasContent = !!(label || icon);
  return (
    <button
      className={hasContent ? "btn btn-ghost" : "btn btn-ghost btn-icon"}
      onClick={handleCopy}
      title={title || text}
      style={{ position: "relative", flexShrink: 0, gap: hasContent ? 5 : 0, fontSize: hasContent ? 12 : undefined, padding: hasContent ? "4px 10px" : undefined }}
    >
      {icon ? icon : copied ? (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M20 6L9 17l-5-5" />
        </svg>
      ) : (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      )}
      {!icon && label && <span style={{ fontWeight: 500 }}>{label}</span>}
    </button>
  );
}

/**
 * 拉取每个 group 的使用统计 + 余额。
 * - usage stats：按 proxy_log.group_key 聚合（`groupUsageApi.statsAll` 单次批量），只含本分组请求，共享平台不重复计入。
 * - balance：关联 platforms 的 est_balance_remaining 求和（平台级属性，无 per-group 概念，维持现状）。
 * load 与 refreshStats 共用，避免两处求和逻辑重复。
 */
async function fetchGroupStats(
  details: GroupDetail[],
  platforms: Platform[],
): Promise<{ statsMap: Record<string, PlatformUsageStats>; balanceMap: Record<number, number> }> {
  const platById = new Map(platforms.map(pp => [pp.id, pp]));
  const statsMap: Record<string, PlatformUsageStats> = {};
  const balanceMap: Record<number, number> = {};
  // usage stats：单次批量 invoke（后端 GROUP BY group_key），消除逐 group N+1 往返。
  // 返回 map 仅含有日志的 group；total_requests > 0 时纳入。
  try {
    const all = await groupUsageApi.statsAll();
    for (const g of details) {
      const s = all[g.group.group_key];
      if (s && s.total_requests > 0) statsMap[g.group.group_key] = s;
    }
  } catch { /* ignore */ }
  // balance：关联平台余额求和（保持平台级语义，无 HTTP）。
  for (const g of details) {
    let balance = 0;
    for (const gp of g.platforms) {
      const est = platById.get(gp.platform.id)?.est_balance_remaining;
      if (typeof est === "number" && est > 0) balance += est;
    }
    if (balance > 0) balanceMap[g.group.id] = balance;
  }
  return { statsMap, balanceMap };
}

/** 分组内嵌组件（供 Platforms 页使用） */
export function GroupsEmbedded({ onNavigate, onGroupsChanged, onCreatePlatform, onToast }: {
  onNavigate?: (id: string, context?: { groupId?: string; groupKey?: string; platformId?: number; platformName?: string }) => void;
  onGroupsChanged?: () => void;
  /** 打开平台创建表单；提供 lockedGroupId = 从某分组 ➕ 触发，预绑该分组且锁定归属。 */
  onCreatePlatform?: (presetGroupIds?: number[], lockedGroupId?: number) => void;
  /** 透传父级 toast setter（快速测试/额度刷新结果反馈）；不传则 usePlatformCards 兜底空函数。 */
  onToast?: (toast: { text: string; ok: boolean } | null) => void;
}) {
  const { t } = useTranslation();
  const [details, setDetails] = useState<GroupDetail[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [groupStats, setGroupStats] = useState<Record<string, PlatformUsageStats>>({});
  // 聚合余额：关联 platforms 的 est_balance_remaining 求和（platformApi.list 已带，无额外 HTTP）。group.id → 余额；缺值不写入。
  const [groupBalance, setGroupBalance] = useState<Record<number, number>>({});
  const [loading, setLoading] = useState(true);
  // 代理端口（proxy_get_settings），构造页面级 base_url；取失败兜底 7890。
  const [proxyPort, setProxyPort] = useState(7890);
  const proxyBaseUrl = `http://127.0.0.1:${proxyPort}/proxy`;

  // Edit mode（8 字段合并为单 reducer）
  const [edit, dispatchEdit] = useReducer(editReducer, EMPTY_EDIT);
  const {
    target: editTarget,
    name: editName,
    mode: editMode,
    platformIds: editPlatformIds,
    mappings: editMappings,
    reqTimeout: editReqTimeout,
    connTimeout: editConnTimeout,
    maxRetries: editMaxRetries,
  } = edit;

  // ── Drag reorder for group list (via shared SortableList @dnd-kit) ──
  const handleReorderGroups = (next: GroupRow[]) => {
    const reordered = next.map(r => r.detail);
    setDetails(reordered);
    groupApi.reorder(reordered.map(d => d.group.id)).catch(console.error);
  };

  // ── Drag reorder for selected platforms (order = routing priority) ──
  // SortableList yields the fully reordered rows; map back to platform ids to persist on save.
  const handleReorderPlatforms = (next: SortablePlatform[]) => {
    dispatchEdit({ type: "patch", patch: { platformIds: next.map(row => row.platformId) } });
  };

  // Create mode
  const [showCreate, setShowCreate] = useState(false);
  const [cName, setCName] = useState("");
  const [cGroupKey, setCGroupKey] = useState("");
  const [cMode, setCMode] = useState<RoutingMode>("health_aware");

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
      const { statsMap, balanceMap } = await fetchGroupStats(d || [], p || []);
      setGroupStats(statsMap);
      setGroupBalance(balanceMap);
    } catch (e) { console.error(e); }
    setLoading(false);
  };

  /** 轻量刷新：更新 platforms（含 est_balance_remaining）+ usage stats + group 聚合，不拉 quota HTTP */
  const refreshStats = async () => {
    try {
      const [d, p] = await Promise.all([groupDetailApi.list(), platformApi.list()]);
      const { statsMap, balanceMap } = await fetchGroupStats(d || [], p || []);
      setGroupStats(statsMap);
      setGroupBalance(balanceMap);
    } catch { /* ignore */ }
  };

  useEffect(() => { load(); }, []);

  // ── 分组展开区平台卡片：复用 PlatformCard + usePlatformCards（与 Platforms 主列表同款） ──
  // 单实例 hook 跨所有分组共享 state（quota/usage/expanded/test 按 platformId 索引）。
  const cards = usePlatformCards({ onNavigate, setToast: onToast });
  // 分组卡片受控展开态（header 点击 + chevron 都驱动此 set）。
  const [expandedGroups, setExpandedGroups] = useState<Set<number>>(new Set());
  const toggleGroupExpanded = (id: number) => setExpandedGroups(prev => {
    const s = new Set(prev); s.has(id) ? s.delete(id) : s.add(id); return s;
  });
  // 分组上下文 card actions：拖拽 no-op（分组内禁拖拽）；启停/删除后 load() 刷新本地 platforms。
  const groupCardActions = useMemo<PlatformCardActions>(() => ({
    onPointerDown: () => {}, onPointerMove: () => {}, onPointerUp: () => {},
    onToggleExpanded: cards.toggleExpanded,
    onRefreshQuota: cards.refreshQuota,
    onToggleEnabled: async (p) => { await cards.handleToggle(p); load(); },
    onEdit: cards.handleEdit,
    onDelete: async (id) => { await cards.handleDelete(id); load(); },
    onViewLogs: cards.handleViewLogs,
    onQuickTest: cards.handleQuickTest,
    onCustomTest: cards.handleCustomTest,
    onFaviconFailed: (id) => cards.onFaviconFailed(prev => new Set(prev).add(id)),
    // handlers 来自 usePlatformCards 的 useCallback（稳定）；load 内联故每次重算——分组展开非热路径，可接受
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [cards, load]);

  // ── 分组展开区平台拖拽（HTML5 DnD，不与 dnd-kit 分组排序冲突；天然支持跨分组移动） ──
  // payload 挂 window 全局，跨组件共享：Platforms 主列表未分组平台也能拖入分组（fromGid=0）
  type DndPayload = { pid: number; fromGid: number };
  const getDnd = (): DndPayload | null => (window as unknown as { __aidogDnd?: DndPayload }).__aidogDnd ?? null;
  const setDnd = (v: DndPayload | null) => {
    const w = window as unknown as { __aidogDnd?: DndPayload };
    if (v) w.__aidogDnd = v; else delete w.__aidogDnd;
  };
  const [dropIndicator, setDropIndicator] = useState<{ gid: number; idx: number } | null>(null);
  // 拖拽悬停的分组（折叠态整体高亮，展开态配合 dropIndicator 精细指示）
  const [dragOverGroup, setDragOverGroup] = useState<number | null>(null);

  const onPlatDragStart = (e: ReactDragEvent, pid: number, gid: number, cardEl: HTMLElement | null) => {
    setDnd({ pid, fromGid: gid });
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(pid)); // Firefox 触发 dragstart 必填
    if (cardEl) e.dataTransfer.setDragImage(cardEl, 12, 12);
  };
  const onPlatDragEnd = () => { setDnd(null); setDropIndicator(null); setDragOverGroup(null); };

  // 基于 clientY 计算 drop 到容器内第 idx 张卡片前（末尾 = 卡片数）
  const computeDropIdx = (zoneEl: HTMLElement, clientY: number): number => {
    const cards = zoneEl.querySelectorAll<HTMLElement>("[data-gp-id]");
    for (let i = 0; i < cards.length; i++) {
      const r = cards[i].getBoundingClientRect();
      if (clientY < r.top + r.height / 2) return i;
    }
    return cards.length;
  };

  const onZoneDragOver = (e: ReactDragEvent, gid: number, zoneEl: HTMLElement) => {
    if (!getDnd()) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    setDragOverGroup(gid);
    const idx = computeDropIdx(zoneEl, e.clientY);
    setDropIndicator(prev => (prev?.gid === gid && prev?.idx === idx) ? prev : { gid, idx });
  };

  const onZoneDrop = (e: ReactDragEvent, gid: number, zoneEl: HTMLElement) => {
    e.preventDefault();
    const payload = getDnd();
    setDnd(null);
    setDropIndicator(null);
    setDragOverGroup(null);
    if (!payload) return;
    const idx = computeDropIdx(zoneEl, e.clientY);
    // 从 details 推导目标分组当前平台顺序（dropzone 已提升到分组 wrapper，fullPlats 不再由调用方传）
    const fullPlats = (details.find(d => d.group.id === gid)?.platforms ?? [])
      .map(gp => platforms.find(pp => pp.id === gp.platform.id))
      .filter((pp): pp is Platform => !!pp);

    if (payload.fromGid === gid) {
      // 组内重排
      const ids = fullPlats.map(p => p.id);
      const fromIdx = ids.indexOf(payload.pid);
      if (fromIdx < 0) return;
      let target = idx;
      if (fromIdx < idx) target = idx - 1; // 移除拖动项后位置左移
      if (target === fromIdx) return;
      const reordered = ids.filter(id => id !== payload.pid);
      reordered.splice(target, 0, payload.pid);
      setDetails(prev => prev.map(d => d.group.id !== gid ? d : {
        ...d,
        platforms: reordered.map((id, i) => {
          const gp = d.platforms.find(g => g.platform.id === id)!;
          return { ...gp, priority: i + 1 };
        }),
      }));
      groupDetailApi.reorderPlatforms(gid, reordered).catch(console.error);
    } else {
      if (payload.fromGid === 0) {
        // 从未分组列表拖入（fromGid=0，无源组）: 构造新明细乐观插入目标组
        const plat = platforms.find(pp => pp.id === payload.pid);
        if (plat) {
          setDetails(prev => prev.map(d => {
            if (d.group.id !== gid) return d;
            const newGp: GroupPlatformDetail = { platform: plat, priority: d.platforms.length + 1, weight: 1 };
            const gps = [...d.platforms];
            gps.splice(Math.min(idx, gps.length), 0, newGp);
            return { ...d, platforms: gps };
          }));
        }
        groupDetailApi.movePlatform(payload.pid, 0, gid)
          .then(() => { load(); onGroupsChanged?.(); })
          .catch((err) => {
            console.error("[aidog-dnd] movePlatform failed", err);
            onToast?.({ text: `加入分组失败: ${err}`, ok: false });
            load(); // 回滚乐观插入
          });
      } else {
        // 跨组移动
        let movedGp: GroupPlatformDetail | undefined;
        setDetails(prev => {
          const next = prev.map(d => {
            if (d.group.id === payload.fromGid) {
              const gps = d.platforms.filter(g => {
                if (g.platform.id === payload.pid) { movedGp = g; return false; }
                return true;
              });
              return { ...d, platforms: gps };
            }
            return d;
          });
          if (!movedGp) return next;
          return next.map(d => {
            if (d.group.id !== gid) return d;
            const newGp = { ...movedGp!, priority: d.platforms.length + 1 };
            const gps = [...d.platforms];
            const insertAt = Math.min(idx, gps.length);
            gps.splice(insertAt, 0, newGp);
            return { ...d, platforms: gps };
          });
        });
        groupDetailApi.movePlatform(payload.pid, payload.fromGid, gid)
          .then(() => load()).catch(console.error);
      }
    }
  };

  // 取代理端口构造 base_url；失败保持兜底 7890。
  useEffect(() => {
    proxyApi.getSettings()
      .then(s => { if (s?.port) setProxyPort(s.port); })
      .catch(() => { /* 兜底 7890 */ });
  }, []);

  // 请求完成后轻量刷新统计（仅本地 DB 查询，不拉 quota HTTP）
  useEffect(() => onProxyLogUpdated(() => { refreshStats(); }), []);

  // ── Edit handlers ──

  const openEdit = (detail: GroupDetail) => {
    dispatchEdit({ type: "open", detail });
  };

  const cancelEdit = () => {
    dispatchEdit({ type: "reset" });
  };

  const saveEdit = async () => {
    if (!editTarget) return;
    try {
      // Update group basic info + inline model mappings
      await groupApi.update({
        id: editTarget.group.id,
        name: editName,
        routing_mode: editMode,
        request_timeout_secs: editReqTimeout,
        connect_timeout_secs: editConnTimeout,
        max_retries: editMaxRetries,
        model_mappings: editMappings,
      });

      // Update platforms
      await groupApi.setPlatforms(
        editTarget.group.id,
        editPlatformIds.map((pid, i) => ({ platform_id: pid, priority: i + 1, weight: 1 })),
      );

      cancelEdit();
      load();
      onGroupsChanged?.();
    } catch (e) {
      console.error(e);
      alert(String(e) || "Failed to save group");
    }
  };

  // ── Create handler ──
  const handleCreateGroup = async () => {
    try {
      await groupApi.create({ name: cName, group_key: cGroupKey.trim() || undefined, routing_mode: cMode });
      setCName(""); setCGroupKey(""); setCMode("failover"); setShowCreate(false);
      load();
      onGroupsChanged?.();
    } catch (e) { console.error(e); }
  };

  const handleDeleteGroup = async (id: number) => {
    try {
      await groupApi.delete(id);
      load();
      onGroupsChanged?.();
    } catch (e) {
      alert(String(e) || "Failed to delete group");
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
      onGroupsChanged?.();
    } catch (e) { console.error(e); }
  };

  const handleDeleteMapping = async (groupId: number, index: number) => {
    const detail = details.find(d => d.group.id === groupId);
    if (!detail) return;
    try {
      const next = detail.model_mappings.filter((_, i) => i !== index);
      await groupApi.update({ id: groupId, model_mappings: next });
      load();
      onGroupsChanged?.();
    } catch (e) { console.error(e); }
  };

  const selectedPlatform = platforms.find(p => p.id === mTargetPlatform);
  const availableModels = selectedPlatform ? allModelValues(selectedPlatform.models) : [];

  // ── Edit page ──
  if (editTarget) {
    const editPlatformOptions = platforms.filter(p => p.enabled);
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
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
          <CopyButton text={editTarget.group.group_key} label={t("group.apiKey", "API Key")} title={t("group.copyApiKeyTitle", "复制 API Key")} />
          <CopyButton text={buildClaudeCommand(editTarget.group.group_key)} icon={<img src={claudeIcon} width={14} height={14} alt="Claude" />} title={t("group.copyCommand", "复制 Claude Code 启动命令")} />
          <CopyButton text={buildCodexCommand(editTarget.group.group_key)} icon={<img src={codexIcon} width={14} height={14} alt="Codex" />} title={t("group.copyCodexCommand", "复制 Codex 命令")} />
          <button className="btn" onClick={cancelEdit}>{t("action.cancel")}</button>
          <button className="btn btn-primary" onClick={saveEdit}
            disabled={!editName}>{t("action.save")}</button>
        </div>

        {/* Basic info */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600, marginBottom: 4 }}>{t("group.basicInfo", "基本信息")}</div>

          {/* Name */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.name", "名称")}</span>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
              value={editName} onChange={e => dispatchEdit({ type: "patch", patch: { name: e.target.value } })} />
          </div>

          {/* Group key（锁定，创建后不可改） */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.groupKey", "密钥")}</span>
            <div style={{ display: "flex", gap: 6, alignItems: "center", minWidth: 0 }}>
              <input className="input" style={{ fontSize: F.body, padding: S.inputPad, opacity: 0.7 }}
                value={editTarget.group.group_key} disabled
                title={t("group.groupKeyLocked", "分组密钥创建后锁定，不可修改")} />
              <CopyButton text={editTarget.group.group_key} title={t("group.copyApiKeyTitle", "复制 API Key")} size={14} />
            </div>
          </div>

          {/* Routing mode */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "start", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)", paddingTop: 6 }}>{t("group.routingMode", "路由模式")}</span>
            <div style={{ display: "flex", flexDirection: "column", gap: 4, minWidth: 0 }}>
              <select className="input" style={{ fontSize: F.body, padding: S.inputPad }}
                value={editMode} onChange={e => dispatchEdit({ type: "patch", patch: { mode: e.target.value as RoutingMode } })}>
                {ROUTING_MODES.map(m => (
                  <option key={m} value={m}>{routingModeLabel(t, m)}</option>
                ))}
              </select>
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{routingModeDesc(t, editMode)}</span>
            </div>
          </div>

          {/* Timeout */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.timeout", "超时")}</span>
            <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
              <input className="input" type="number" min={0} placeholder={t("group.reqTimeout", "请求(s)")}
                value={editReqTimeout || ""} onChange={e => dispatchEdit({ type: "patch", patch: { reqTimeout: Math.max(0, Number(e.target.value)) } })}
                style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
              <input className="input" type="number" min={0} placeholder={t("group.connTimeout", "连接(s)")}
                value={editConnTimeout || ""} onChange={e => dispatchEdit({ type: "patch", patch: { connTimeout: Math.max(0, Number(e.target.value)) } })}
                style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{t("group.timeoutDefault", "0 = 系统默认（秒）")}</span>
            </div>
          </div>

          {/* Max retries（多平台失败逐个重试上限） */}
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("group.maxRetries", "最大重试")}</span>
            <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
              <input className="input" type="number" min={0} max={10}
                value={editMaxRetries}
                onChange={e => dispatchEdit({ type: "patch", patch: { maxRetries: Math.max(0, Number(e.target.value)) } })}
                style={{ width: 80, fontSize: F.body, padding: S.inputPad }} />
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }}>{t("group.maxRetriesHint", "0 = 不重试，只试 1 个平台")}</span>
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
            <SortableList<SortablePlatform>
              items={editPlatformIds.map(pid => ({ id: String(pid), platformId: pid }))}
              onReorder={handleReorderPlatforms}
              renderItem={(row, handle) => {
                const pid = row.platformId;
                const i = editPlatformIds.indexOf(pid);
                const p = platforms.find(pp => pp.id === pid);
                if (!p) return null;
                return (
                  <div style={{
                    display: "flex", alignItems: "center", gap: 10,
                    padding: "8px 12px", borderRadius: "var(--radius-sm)",
                    background: "var(--bg-glass)",
                    border: "1px solid var(--border)",
                    marginBottom: 4,
                    transition: "opacity 0.15s, border-color 0.15s",
                  }}>
                    <span
                      ref={handle.ref}
                      {...handle.attributes}
                      {...handle.listeners}
                      title={t("group.dragToReorder", "拖动排序")}
                      style={{
                        cursor: "grab", color: "var(--text-tertiary)", fontSize: 14,
                        lineHeight: 1, userSelect: "none", flexShrink: 0, touchAction: "none",
                      }}
                    >⠿</span>
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
                        dispatchEdit({ type: "patch", patch: { platformIds: ids } });
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
                        dispatchEdit({ type: "patch", patch: { platformIds: ids } });
                      }}>
                      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                        <path d="M5 8V2M2 5l3 3 3-3" />
                      </svg>
                    </button>
                    <button type="button" onClick={() => dispatchEdit({ type: "patch", patch: { platformIds: editPlatformIds.filter(id => id !== pid) } })} style={{
                      background: "none", border: "none", cursor: "pointer",
                      color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1,
                    }}><IconClose size={12} /></button>
                  </div>
                );
              }}
            />
          </div>
          {/* Add platform */}
          {editPlatformIds.length < editPlatformOptions.length && (
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <select className="input" style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                onChange={e => {
                  const pid = Number(e.target.value);
                  if (e.target.value && !editPlatformIds.includes(pid)) {
                    dispatchEdit({ type: "patch", patch: { platformIds: [...editPlatformIds, pid] } });
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
                      dispatchEdit({ type: "patch", patch: { mappings: ms } });
                    }} />
                  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round">
                    <path d="M2 6h8M8 4l2 2-2 2" />
                  </svg>
                  <select className="input" style={{ fontSize: F.hint, padding: "6px 10px", width: 140, flexShrink: 0 }}
                    value={m.target_platform_id || ""}
                    onChange={e => {
                      const ms = [...editMappings];
                      ms[i] = { ...ms[i], target_platform_id: e.target.value === "" ? 0 : Number(e.target.value), target_model: "" };
                      dispatchEdit({ type: "patch", patch: { mappings: ms } });
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
                        dispatchEdit({ type: "patch", patch: { mappings: ms } });
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
                        dispatchEdit({ type: "patch", patch: { mappings: ms } });
                      }} />
                  )}
                  <button type="button" onClick={() => dispatchEdit({ type: "patch", patch: { mappings: editMappings.filter((_, j) => j !== i) } })} style={{
                    background: "none", border: "none", cursor: "pointer",
                    color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1, flexShrink: 0,
                  }}><IconClose size={12} /></button>
                </div>
              );
            })}

            <button type="button" className="btn btn-ghost" style={{ fontSize: F.hint, padding: "6px 12px", alignSelf: "flex-start" }}
              onClick={() => dispatchEdit({ type: "patch", patch: { mappings: [...editMappings, { source_model: "", target_platform_id: 0, target_model: "", request_timeout_secs: 0, connect_timeout_secs: 0 }] } })}>
              + {t("mapping.add", "添加映射")}
            </button>
          </div>
        </div>

        {/* Middleware rules (group scope) */}
        <div className="glass-surface" style={{ padding: S.pad, display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ fontSize: F.label, fontWeight: 600 }}>{t("middleware.groupRules", "分组中间件规则")}</div>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: -8 }}>
            {t("middleware.groupRulesHint", "仅本分组生效，就近覆盖全局同类型规则")}
          </div>
          <MiddlewareRulesPanel scope="group" scopeRef={editTarget.group.group_key} embedded />
        </div>
      </div>
    );
  }

  // ── List view ──
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* 子区块标题 + 操作栏 */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
          <span style={{ fontSize: 18, fontWeight: 700 }}>{t("page.groups")}</span>
          {details.length > 0 && (
            <span style={{ fontSize: 13, color: "var(--text-tertiary)" }}>
              {details.length} {t("nav.groups").toLowerCase()}
            </span>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          {/* 代理 base_url：只读小字 + 复制按钮 */}
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <code style={{
              fontSize: 12, color: "var(--text-secondary)", background: "var(--bg-glass)",
              padding: "4px 8px", borderRadius: "var(--radius-sm)", whiteSpace: "nowrap",
            }}>{proxyBaseUrl}</code>
            <CopyButton text={proxyBaseUrl} label={t("group.copyBaseUrl", "复制代理地址")}
              title={t("group.copyBaseUrlTitle", "复制代理 base_url")} />
          </div>
          <button className="btn btn-primary" onClick={() => setShowCreate(true)}>
            + {t("group.add")}
          </button>
          {onCreatePlatform && (
            <button className="btn" onClick={() => onCreatePlatform()}>
              + {t("platform.add", "添加平台")}
            </button>
          )}
        </div>
      </div>

      {/* Create Group Form */}
      {showCreate && (
        <div className="glass-surface animate-fade-in" style={{
          padding: 20,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}>
          <input className="input" placeholder={t("group.name", "分组名称")} value={cName}
            onChange={(e) => setCName(e.target.value)} />
          <div style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: -4 }}>
            {t("group.nameHint", "分组显示名（中文可读），用于界面展示。")}
          </div>
          <input className="input" placeholder={t("group.groupKey", "分组密钥（留空自动生成）")} value={cGroupKey}
            onChange={(e) => setCGroupKey(e.target.value.replace(/[^\w-]/g, ""))} />
          <div style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: -4 }}>
            {t("group.groupKeyHint", "分组密钥（= API Key / 路由识别键）。留空自动生成；创建后锁定不可修改。")}
          </div>
          <select className="input" value={cMode} onChange={(e) => setCMode(e.target.value as RoutingMode)}>
            {ROUTING_MODES.map(m => (
              <option key={m} value={m}>{routingModeLabel(t, m)}</option>
            ))}
          </select>
          <div style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: -4 }}>{routingModeDesc(t, cMode)}</div>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button className="btn" onClick={() => setShowCreate(false)}>{t("action.cancel")}</button>
            <button className="btn btn-primary" onClick={handleCreateGroup}
              disabled={!cName}>{t("action.create")}</button>
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
          <SortableList<GroupRow>
            items={details.map(d => ({ id: String(d.group.id), detail: d }))}
            onReorder={handleReorderGroups}
            renderItem={(row, handle) => {
            const { group, platforms: gps, model_mappings } = row.detail;
            const i = details.findIndex(d => d.group.id === group.id);
            const u = groupStats[group.group_key];
            const balance = groupBalance[group.id];
            const totalTokens = u ? u.total_input_tokens + u.total_output_tokens : 0;
            const sRate = u ? calcSuccessRate(u.success_count, u.total_requests) : 0;

            const header = (
              <div style={{ display: "flex", flexDirection: "column", gap: 10, minWidth: 0 }}>
                {/* ── 行 1：身份 + 快操作 ── */}
                <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0 }}>
                {/* Drag handle */}
                <span
                  ref={handle.ref}
                  {...handle.attributes}
                  {...handle.listeners}
                  className={`drag-handle drag-handle-inline${handle.isDragging ? " is-active" : ""}`}
                  title={t("group.dragToReorder", "拖动排序")}
                  style={{ touchAction: "none", flexShrink: 0, display: "inline-flex" }}
                  onClick={e => e.stopPropagation()}
                >
                  <svg width="14" height="20" viewBox="0 0 14 20" fill="currentColor"><circle cx="4" cy="3" r="1.8"/><circle cx="4" cy="10" r="1.8"/><circle cx="4" cy="17" r="1.8"/><circle cx="10" cy="3" r="1.8"/><circle cx="10" cy="10" r="1.8"/><circle cx="10" cy="17" r="1.8"/></svg>
                </span>
                {/* Group icon：单平台跟随平台 logo */}
                <GroupIcon gps={gps} group={group} />
                {/* Name + path + routing + platform count */}
                <div
                  style={{ flex: 1, minWidth: 0, cursor: "pointer" }}
                  onClick={() => { if (!handle.isDragging) toggleGroupExpanded(group.id); }}
                >
                  <div style={{ fontWeight: 600, fontSize: 14, display: "flex", alignItems: "center", gap: 6 }}>
                    {group.name}
                    {group.auto_from_platform && (
                      <span className="badge badge-muted" style={{ fontSize: 10, padding: "0 5px", fontWeight: 500 }}>auto</span>
                    )}
                  </div>
                  <div className="text-secondary" style={{ fontSize: 12, display: "flex", gap: 8, marginTop: 1, alignItems: "center", flexWrap: "wrap" }}>
                    <span className="badge badge-muted" style={{ padding: "0 6px" }}>
                      {routingModeLabel(t, group.routing_mode)}
                    </span>
                    {gps.length > 0 && (
                      <span className="text-tertiary">{gps.length} {t("group.platforms", "平台")}</span>
                    )}
                  </div>
                </div>
                {/* Quick actions */}
                <CopyButton text={group.group_key} title={t("group.copyApiKeyTitle", "复制 API Key")} size={14} />
                <CopyButton text={buildClaudeCommand(group.group_key)} icon={<img src={claudeIcon} width={14} height={14} alt="Claude" />} title={t("group.copyCommand", "复制 Claude Code 启动命令")} size={14} />
                <CopyButton text={buildCodexCommand(group.group_key)} icon={<img src={codexIcon} width={14} height={14} alt="Codex" />} title={t("group.copyCodexCommand", "复制 Codex 命令")} size={14} />
                <button className="btn btn-ghost btn-icon" onClick={e => { e.stopPropagation(); onNavigate?.("stats", { groupId: String(group.id), groupKey: group.group_key }); }} title={t("group.viewStats", "查看统计")}>
                  <svg width="14" height="14" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M3 15V8M7 15V5M11 15V10M15 15V3" />
                  </svg>
                </button>
                {onCreatePlatform && (
                  <button className="btn btn-ghost btn-icon" onClick={e => { e.stopPropagation(); onCreatePlatform([group.id], group.id); }} title={t("group.addPlatformToGroup", "在此分组添加平台")}>
                    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M7 2v10M2 7h10" />
                    </svg>
                  </button>
                )}
                <button className="btn btn-ghost btn-icon" onClick={e => { e.stopPropagation(); openEdit({ group, platforms: gps, model_mappings }); }} title={t("action.edit", "编辑")}>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                  </svg>
                </button>
                {(!group.auto_from_platform || gps.length === 0) && (
                  <button className="btn btn-ghost btn-icon btn-danger" onClick={(e) => { e.stopPropagation(); handleDeleteGroup(group.id); }}>
                    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                    </svg>
                  </button>
                )}
                </div>
                {/* ── 行 2：统计 + 余额 ── */}
                {(u || balance != null) && (
                  <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap", paddingLeft: 26 }}>
                {/* Aggregate stats chips */}
                {u && (
                  <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
                    <StatChip icon={<IconBolt size={13} />} value={formatNumber(totalTokens)} label="tokens" />
                    <StatChip icon={<IconCost size={13} />} value={`$${formatCost(u.total_cost)}`} label="cost" level={costLevel(u.total_cost)} />
                    {u.total_requests > 0 && (
                      <StatChip icon={<IconCheck size={13} />} value={formatPercent(sRate, 0)} label="ok"
                        level={successRateLevel(sRate, u.total_requests)} />
                    )}
                  </div>
                )}
                {/* Aggregate balance */}
                {balance != null && (
                  <div style={{ minWidth: 90, flexShrink: 0 }}>
                    <BalanceBar remaining={balance} showTotal={false} />
                  </div>
                )}
                  </div>
                )}
              </div>
            );

            return (
              <div
                className="animate-fade-in"
                style={{ animationDelay: `${i * 60}ms` }}
                onDragOver={(e) => onZoneDragOver(e, group.id, e.currentTarget as HTMLElement)}
                onDrop={(e) => onZoneDrop(e, group.id, e.currentTarget as HTMLElement)}
                onDragLeave={(e) => {
                  const related = e.relatedTarget as Node | null;
                  if (!related || !(e.currentTarget as HTMLElement).contains(related)) {
                    setDropIndicator(prev => prev?.gid === group.id ? null : prev);
                    setDragOverGroup(prev => prev === group.id ? null : prev);
                  }
                }}
              >
                <CompactCard
                  header={header}
                  expanded={expandedGroups.has(group.id)}
                  onToggle={(next) => setExpandedGroups(prev => {
                    const s = new Set(prev); next ? s.add(group.id) : s.delete(group.id); return s;
                  })}
                  toggleLabel={t("group.toggleDetails", "展开/收起明细")}
                  style={handle.isDragging
                    ? { opacity: 0.5 }
                    : dragOverGroup === group.id
                      ? { outline: "2px solid var(--accent)", outlineOffset: 2 }
                      : undefined}
                >
                  {(
                    <div style={{ display: "flex", flexDirection: "column", gap: 10 }} onClick={e => e.stopPropagation()}>
                      {/* 关联平台：完整 PlatformCard（同 Platforms 主列表），点卡片就地展开详情 */}
                      {gps.length > 0 && (() => {
                        const fullPlats = gps
                          .map(gp => platforms.find(pp => pp.id === gp.platform.id))
                          .filter((pp): pp is Platform => !!pp);
                        return (
                          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                            {fullPlats.map((p, idx) => (
                              <Fragment key={p.id}>
                                {dropIndicator?.gid === group.id && dropIndicator.idx === idx && (
                                  <div style={{ height: 2, background: "var(--accent)", borderRadius: 1, margin: "-3px 0", opacity: 0.7 }} />
                                )}
                                <div style={{ display: "flex", gap: 4, alignItems: "stretch" }}>
                                  {/* HTML5 拖拽把手：组内排序 + 跨分组移动 */}
                                  <span
                                    draggable
                                    onDragStart={(e) => {
                                      const cardEl = (e.currentTarget as HTMLElement).parentElement?.querySelector("[data-gp-id]") as HTMLElement | null;
                                      onPlatDragStart(e, p.id, group.id, cardEl);
                                    }}
                                    onDragEnd={onPlatDragEnd}
                                    className="drag-handle drag-handle-inline"
                                    style={{ cursor: "grab", display: "inline-flex", alignItems: "center", flexShrink: 0, alignSelf: "center" }}
                                    title={t("group.dragPlatform", "拖拽排序 / 移动到其他分组")}
                                  >
                                    <svg width="12" height="18" viewBox="0 0 14 20" fill="currentColor"><circle cx="4" cy="3" r="1.8"/><circle cx="4" cy="10" r="1.8"/><circle cx="4" cy="17" r="1.8"/><circle cx="10" cy="3" r="1.8"/><circle cx="10" cy="10" r="1.8"/><circle cx="10" cy="17" r="1.8"/></svg>
                                  </span>
                                  <div data-gp-id={p.id} style={{ flex: 1, minWidth: 0 }}>
                                    <PlatformCard
                                      platform={p}
                                      index={idx}
                                      isDragging={false}
                                      dragActive={false}
                                      quota={computeQuotaDisplay(p, cards.quotaMap[p.id], !!cards.quotaRealIds[p.id])}
                                      refreshing={!!cards.quotaRefreshing[p.id]}
                                      usage={cards.usageMap[p.id]}
                                      expanded={cards.expandedIds.has(p.id)}
                                      manualResult={cards.testResults[p.id]}
                                      testing={cards.testingId === p.id}
                                      faviconFailed={cards.faviconFailed.has(p.id)}
                                      actions={groupCardActions}
                                      draggable={false}
                                    />
                                  </div>
                                </div>
                              </Fragment>
                            ))}
                            {dropIndicator?.gid === group.id && dropIndicator.idx === fullPlats.length && (
                              <div style={{ height: 2, background: "var(--accent)", borderRadius: 1, margin: "-3px 0", opacity: 0.7 }} />
                            )}
                          </div>
                        );
                      })()}


                      {/* Model Mappings */}
                      {model_mappings.length > 0 && (
                        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                          {model_mappings.map((m, mi) => (
                            <div key={mi} style={{
                              display: "flex", alignItems: "center", gap: 8, fontSize: 12,
                              padding: "6px 10px", borderRadius: "var(--radius-sm)",
                              background: "var(--bg-glass)", border: "1px solid var(--border)",
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
                      <button className="btn btn-ghost" style={{ fontSize: 12, gap: 4, padding: "4px 8px", color: "var(--text-secondary)", alignSelf: "flex-start" }}
                        onClick={(e) => { e.stopPropagation(); setMappingGroupId(mappingGroupId === group.id ? null : group.id); }}>
                        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                          <path d="M6 2v8M2 6h8" />
                        </svg>
                        {t("mapping.add")}
                      </button>

                      {mappingGroupId === group.id && (
                        <div className="animate-fade-in" style={{
                          paddingTop: 10, borderTop: "1px solid var(--border)",
                          display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap",
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
                  )}
                </CompactCard>
              </div>
            );
            }}
          />
        </div>
      )}
    </div>
  );
}
