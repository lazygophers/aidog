// usePlatformsState — Platforms 主组件的 state + handlers 编排层。
// ponytail: 收编 Platforms 主组件除 quota 子系统（usePlatformQuota）+ form 子系统（usePlatformForm）
//   外的全部 list/drag/CRUD/effect 逻辑。form state + form handlers 已抽到 usePlatformForm.ts
//   （经 listDeps 注入 list 侧依赖保持闭包共享）。本 hook 负责 list 态 + 派生 + effects + return 组装。
//
// 子组件消费：PlatformEditForm（编辑态）+ PlatformListView（列表态）通过 props 拿本 hook 返回值。
import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  platformApi, modelTestApi, groupDetailApi, schedulingApi,
  onProxyLogUpdated,
  type Platform, type PlatformStatus, type Protocol, type PlatformEndpoint,
  type PlatformUsageStats, type LastTestResult,
  type SchedulingBreakerSettings, type GroupDetail, type SharePlatform,
  type ModelSlot, type MockConfig, type NewApiConfig, type ManualBudget,
  type TimeModelRule,
} from "../../services/api";
import { pinyinMatch } from "../../utils/pinyin";
import { type SmartPasteApplyResult } from "../../components/platforms/SmartPasteModal";
import { usePlatformQuota, getPrimaryBaseUrl } from "./usePlatformQuota";
import { usePlatformForm } from "./usePlatformForm";
import { type PeakWindow } from "../../domains/platforms";
import { setUiExtra } from "../../services/api/ui_extra";

// ponytail: 读 platform.extra JSON 内 _ui_expand_plat bool（缺/解析失败→false）。跨会话展开态持久化。
function readExtraExpanded(extra: string | undefined | null): boolean {
  if (!extra) return false;
  try { return JSON.parse(extra)._ui_expand_plat === true; } catch { return false; }
}

export interface PlatformsStateParams {
  onNavigate?: (id: string, context?: { platformId?: number; platformName?: string; duplicate?: boolean }) => void;
  initialFilter?: { platformId?: number; platformName?: string; duplicate?: boolean };
  /** GroupsEmbedded reload 命令入口（保存/删除/清理后调用，确定性重建分组卡 platforms 态）。 */
  groupsReloadRef: React.MutableRefObject<(() => void) | null>;
}

export interface PlatformsState extends PlatformsStateParams {
  t: TFunction;
  // ── list state ──
  platforms: Platform[];
  setPlatforms: React.Dispatch<React.SetStateAction<Platform[]>>;
  platformsEpochRef: React.MutableRefObject<number>;
  usageMap: Record<number, PlatformUsageStats>;
  setUsageMap: React.Dispatch<React.SetStateAction<Record<number, PlatformUsageStats>>>;
  usageLoading: boolean;
  testResults: Record<number, "ok" | "fail">;
  setTestResults: React.Dispatch<React.SetStateAction<Record<number, "ok" | "fail">>>;
  lastTestMap: Record<number, LastTestResult>;
  setLastTestMap: React.Dispatch<React.SetStateAction<Record<number, LastTestResult>>>;
  testingId: number | null;
  setTestingId: React.Dispatch<React.SetStateAction<number | null>>;
  loading: boolean;
  progressiveCount: { total: number; active: number } | null;
  setProgressiveCount: React.Dispatch<React.SetStateAction<{ total: number; active: number } | null>>;
  // ── card view state ──
  faviconFailed: Set<number>;
  setFaviconFailed: React.Dispatch<React.SetStateAction<Set<number>>>;
  expandedIds: Set<number>;
  toggleExpanded: (id: number, next: boolean) => void;
  // ── drag reorder state ──
  platDrag: { from: number; to: number } | null;
  platListRef: React.RefObject<HTMLDivElement | null>;
  handlePlatPointerDown: (e: React.PointerEvent, index: number) => void;
  handlePlatPointerMove: (e: React.PointerEvent) => void;
  handlePlatPointerUp: () => void;
  // ── group drag state ──
  groupDrag: { pid: number; pname: string; x: number; y: number } | null;
  onStandaloneGroupPointerDown: (e: React.PointerEvent, p: Platform) => void;
  onStandaloneGroupPointerMove: (e: React.PointerEvent) => void;
  onStandaloneGroupPointerUp: (e: React.PointerEvent) => void;
  // ── quota subsystem ──
  quota: ReturnType<typeof usePlatformQuota>;
  // ── membership / groups ──
  platformMembership: Map<number, string[]>;
  groupDetails: GroupDetail[];
  setGroupDetails: React.Dispatch<React.SetStateAction<GroupDetail[]>>;
  handleGroupsChanged: () => Promise<void>;
  /** 平台被删后全量 refetch platforms state（独立信号，与 onGroupsChanged 分组刷新分离）。
   *  - 触发点：Groups 页 confirmDeletePlatform（删平台入口之一）成功后经父级 onPlatformDeleted 回调调本方法。
   *  - 不复用 onGroupsChanged：后者语义「分组结构变更 → 刷 groupDetails」，扩它刷 platforms 会污染其他调用点
   *    （group 增删 / 拖拽 / 映射变更）。本方法语义专一「platform 被删 → 刷 platforms state」。
   *  - 全量 refetch 非乐观 filter：乐观 filter 快但需保后端真删，且 groupDetails 需另刷。全量 refetch 一次 RPC
   *    同时刷 platforms + 触发派生层（membership/standalonePlatforms）重算，语义清晰。多一次 RPC（~10ms）可接受。 */
  refreshPlatforms: () => Promise<void>;
  // ── standalone (未分组 + 搜索) ──
  standalonePlatforms: Platform[];
  searchQuery: string;
  setSearchQuery: React.Dispatch<React.SetStateAction<string>>;
  // ── derived counts ──
  enabledCount: number;
  headerActive: number;
  headerTotal: number;
  // ── form state ──
  editing: Platform | null;
  setEditing: React.Dispatch<React.SetStateAction<Platform | null>>;
  showForm: boolean;
  setShowForm: React.Dispatch<React.SetStateAction<boolean>>;
  showPaste: boolean;
  setShowPaste: React.Dispatch<React.SetStateAction<boolean>>;
  pasteInitialText: string | undefined;
  setPasteInitialText: React.Dispatch<React.SetStateAction<string | undefined>>;
  shareData: { share: SharePlatform; name: string } | null;
  setShareData: React.Dispatch<React.SetStateAction<{ share: SharePlatform; name: string } | null>>;
  toast: { text: string; ok: boolean } | null;
  setToast: React.Dispatch<React.SetStateAction<{ text: string; ok: boolean } | null>>;
  testingPlatform: Platform | null;
  setTestingPlatform: React.Dispatch<React.SetStateAction<Platform | null>>;
  groupFullscreen: boolean;
  setGroupFullscreen: React.Dispatch<React.SetStateAction<boolean>>;
  showKey: boolean;
  setShowKey: React.Dispatch<React.SetStateAction<boolean>>;
  name: string; setName: React.Dispatch<React.SetStateAction<string>>;
  protocol: Protocol; setProtocol: React.Dispatch<React.SetStateAction<Protocol>>;
  codingPlan: boolean; setCodingPlan: React.Dispatch<React.SetStateAction<boolean>>;
  apiKey: string; setApiKey: React.Dispatch<React.SetStateAction<string>>;
  batchPreviewKeys: string[] | null;
  setBatchPreviewKeys: React.Dispatch<React.SetStateAction<string[] | null>>;
  handleApiKeyChange: (v: string) => void;
  confirmBatchCreate: () => Promise<void>;
  cancelBatchPreview: () => void;
  previewNames: string[];
  models: Record<ModelSlot, string>; setModels: React.Dispatch<React.SetStateAction<Record<ModelSlot, string>>>;
  availableModels: string[]; setAvailableModels: React.Dispatch<React.SetStateAction<string[]>>;
  endpoints: PlatformEndpoint[]; setEndpoints: React.Dispatch<React.SetStateAction<PlatformEndpoint[]>>;
  activeDropdown: ModelSlot | null; setActiveDropdown: React.Dispatch<React.SetStateAction<ModelSlot | null>>;
  showClaudeConfig: boolean; setShowClaudeConfig: React.Dispatch<React.SetStateAction<boolean>>;
  claudeConfigJson: string; setClaudeConfigJson: React.Dispatch<React.SetStateAction<string>>;
  globalClaudeConfig: Record<string, any>; setGlobalClaudeConfig: React.Dispatch<React.SetStateAction<Record<string, any>>>;
  extra: string; setExtra: React.Dispatch<React.SetStateAction<string>>;
  mockConfig: MockConfig; setMockConfig: React.Dispatch<React.SetStateAction<MockConfig>>;
  newApiConfig: NewApiConfig; setNewApiConfig: React.Dispatch<React.SetStateAction<NewApiConfig>>;
  manualBudgets: ManualBudget[]; setManualBudgets: React.Dispatch<React.SetStateAction<ManualBudget[]>>;
  breakerFailureThreshold: string; setBreakerFailureThreshold: React.Dispatch<React.SetStateAction<string>>;
  breakerOpenSecs: string; setBreakerOpenSecs: React.Dispatch<React.SetStateAction<string>>;
  breakerHalfOpenMax: string; setBreakerHalfOpenMax: React.Dispatch<React.SetStateAction<string>>;
  breakerDefaults: SchedulingBreakerSettings | null;
  peakHours: PeakWindow[]; setPeakHours: React.Dispatch<React.SetStateAction<PeakWindow[]>>;
  peakHoursTz: "local" | "utc"; setPeakHoursTz: React.Dispatch<React.SetStateAction<"local" | "utc">>;
  disableDuringPeak: boolean; setDisableDuringPeak: React.Dispatch<React.SetStateAction<boolean>>;
  timeModels: TimeModelRule[]; setTimeModels: React.Dispatch<React.SetStateAction<TimeModelRule[]>>;
  autoGroup: boolean; setAutoGroup: React.Dispatch<React.SetStateAction<boolean>>;
  joinGroupIds: number[]; setJoinGroupIds: React.Dispatch<React.SetStateAction<number[]>>;
  levelPriority: number; setLevelPriority: React.Dispatch<React.SetStateAction<number>>;
  expiresAt: number; setExpiresAt: React.Dispatch<React.SetStateAction<number>>;
  expiryEnabled: boolean; setExpiryEnabled: React.Dispatch<React.SetStateAction<boolean>>;
  lockedGroupId: number | null; setLockedGroupId: React.Dispatch<React.SetStateAction<number | null>>;
  fetching: boolean; setFetching: React.Dispatch<React.SetStateAction<boolean>>;
  fetchError: string; setFetchError: React.Dispatch<React.SetStateAction<string>>;
  saveError: string; setSaveError: React.Dispatch<React.SetStateAction<string>>;
  // ── derived form flags ──
  isMock: boolean;
  isPassthrough: boolean;
  keyOptional: boolean;
  apiKeyMissing: boolean;
  uniqueGroupInfo: { show: boolean; groupId: number | null; isAuto: boolean };
  // ── handlers ──
  load: () => Promise<void>;
  refreshStats: () => Promise<void>;
  resetForm: () => void;
  openCreatePlatform: (presetGroupIds?: number[], lockGid?: number) => void;
  handleEdit: (p: Platform) => Promise<void>;
  handleDuplicate: (p: Platform) => Promise<void>;
  handleProtocolChange: (newProtocol: Protocol, newCodingPlan?: boolean) => void | Promise<void>;
  handleModelChange: (slot: ModelSlot, value: string) => void;
  handleModelSelect: (slot: ModelSlot, value: string) => void;
  handleFetchModels: () => Promise<void>;
  handleFillAll: () => void;
  buildModelsPayload: () => Record<string, string | undefined> | undefined;
  handleSave: () => Promise<void>;
  handleDelete: (id: number) => Promise<void>;
  handleToggle: (p: Platform) => Promise<void>;
  handleQuickTest: (p: Platform) => Promise<void>;
  handleShare: (p: Platform) => Promise<void>;
  handleViewLogs: (p: Platform) => void;
  applyPaste: (r: SmartPasteApplyResult) => Promise<void>;
  handlePurgeDisabled: () => Promise<void>;
  runBatchCreateFromPaste: (keys: string[], baseName?: string, effectiveEndpoints?: PlatformEndpoint[], effectiveProtocol?: Protocol) => Promise<void>;
  /** 主 URL 推导 helper（form header desc + fetch models 回退链共用） */
  getPrimaryBaseUrl: (proto: Protocol, eps: PlatformEndpoint[]) => string;
}

export function usePlatformsState(params: PlatformsStateParams): PlatformsState {
  const { t } = useTranslation();
  const { onNavigate, initialFilter, groupsReloadRef } = params;

  // ════════════ LIST STATE ════════════
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [progressiveCount, setProgressiveCount] = useState<{ total: number; active: number } | null>(null);
  const [usageMap, setUsageMap] = useState<Record<number, PlatformUsageStats>>({});
  const [usageLoading, setUsageLoading] = useState(false);
  const [testResults, setTestResults] = useState<Record<number, "ok" | "fail">>({});
  const [lastTestMap, setLastTestMap] = useState<Record<number, LastTestResult>>({});
  const [testingId, setTestingId] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);

  // 局部刷新守卫：每次本地乐观写操作（保存/删除/清理）自增 epoch；在途的 load()/refreshStats
  //   captureEpoch 后异步返回时若 epoch 已变，跳过 setPlatforms(list) 整列表覆盖，防慢后端晚到回弹
  //   （mount-fetch-late-resolve-overwrites-optimistic 坑）。
  const platformsEpochRef = useRef(0);

  // ════════════ CARD VIEW STATE ════════════
  /** favicon 加载失败的平台 ID 集合（回退到文字缩写） */
  const [faviconFailed, setFaviconFailed] = useState<Set<number>>(new Set());
  /** 列表卡片已展开（显 endpoints/模型明细）的平台 ID 集合 */
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());
  // ponytail: per-id debounce timer，连续 toggle 仅末次写 DB（300ms）。无 useDebounce hook → 内联 setTimeout。
  const expandDebounceRef = useRef<Record<number, ReturnType<typeof setTimeout>>>({});
  const toggleExpanded = (id: number, next: boolean) => {
    setExpandedIds(prev => {
      const s = new Set(prev);
      if (next) s.add(id); else s.delete(id);
      return s;
    });
    const timers = expandDebounceRef.current;
    if (timers[id]) clearTimeout(timers[id]);
    timers[id] = setTimeout(() => {
      delete timers[id];
      setUiExtra("platform", id, "_ui_expand_plat", next).catch(console.error);
    }, 300);
  };

  // ════════════ DRAG REORDER ════════════
  const [platDrag, setPlatDrag] = useState<{ from: number; to: number } | null>(null);
  const platListRef = useRef<HTMLDivElement>(null);
  const platDragStartRef = useRef<{ y: number; index: number } | null>(null);
  const platDidDragRef = useRef(false);
  // 拖拽 geometry 计算 rAF 节流：每帧最多算一次，避免逐 pointermove 全列 getBoundingClientRect
  const platDragRafRef = useRef<number | null>(null);
  const platDragYRef = useRef(0);

  const handlePlatPointerDown = (e: React.PointerEvent, index: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    platDragStartRef.current = { y: e.clientY, index };
  };

  // rAF 内执行：基于最新 clientY 重算插入位置
  const computeDragTarget = (clientY: number) => {
    const start = platDragStartRef.current;
    if (!start) return;
    if (!platDrag) {
      if (Math.abs(clientY - start.y) < 5) return;
      setPlatDrag({ from: start.index, to: start.index });
      platDidDragRef.current = true;
    }
    if (!platListRef.current) return;
    const cards = platListRef.current.querySelectorAll<HTMLElement>("[data-platform-id]");
    let newTo = cards.length;
    for (let i = 0; i < cards.length; i++) {
      const rect = cards[i].getBoundingClientRect();
      if (clientY < rect.top + rect.height / 2) { newTo = i; break; }
    }
    setPlatDrag(d => d ? { ...d, to: newTo } : null);
  };

  const handlePlatPointerMove = (e: React.PointerEvent) => {
    if (!platDragStartRef.current) return;
    platDragYRef.current = e.clientY; // 始终记录最新位置
    if (platDragRafRef.current !== null) return; // 本帧已排程，下一帧用最新 Y
    platDragRafRef.current = requestAnimationFrame(() => {
      platDragRafRef.current = null;
      computeDragTarget(platDragYRef.current);
    });
  };

  const handlePlatPointerUp = () => {
    if (platDragRafRef.current !== null) {
      cancelAnimationFrame(platDragRafRef.current);
      platDragRafRef.current = null;
    }
    if (platDrag) {
      const effectiveTo = platDrag.from < platDrag.to ? platDrag.to - 1 : platDrag.to;
      if (platDrag.from !== effectiveTo) {
        // 仅在未分组平台子集内重排（platDrag from/to 均为 standalone 索引）。
        const reordered = [...standalonePlatforms];
        const [moved] = reordered.splice(platDrag.from, 1);
        reordered.splice(effectiveTo, 0, moved);
        // 重建 platforms：已分组平台原位，未分组按新序填回（保 sort_order 全局一致）。
        let si = 0;
        setPlatforms(platforms.map(p => platformMembership.has(p.id) ? p : reordered[si++]));
        platformApi.reorder(reordered.map(pp => pp.id)).catch(console.error);
      }
    }
    setPlatDrag(null);
    platDragStartRef.current = null;
    setTimeout(() => { platDidDragRef.current = false; }, 50);
  };

  // ════════════ GROUP DRAG (pointer-based, 绕 WKWebView 跨区域 DnD 失效) ════════════
  const [groupDrag, setGroupDrag] = useState<{ pid: number; pname: string; x: number; y: number } | null>(null);
  const groupHighlightEl = useRef<HTMLElement | null>(null);
  const clearGroupHighlight = () => {
    if (groupHighlightEl.current) {
      groupHighlightEl.current.style.outline = "";
      groupHighlightEl.current.style.outlineOffset = "";
      groupHighlightEl.current = null;
    }
  };
  const findGroupAt = (x: number, y: number): { el: HTMLElement; gid: number } | null => {
    const el = document.elementFromPoint(x, y) as HTMLElement | null;
    const groupEl = el?.closest("[data-group-id]") as HTMLElement | null;
    if (!groupEl) return null;
    const gid = Number(groupEl.getAttribute("data-group-id"));
    return Number.isFinite(gid) && gid > 0 ? { el: groupEl, gid } : null;
  };
  const onStandaloneGroupPointerDown = (e: React.PointerEvent, p: Platform) => {
    if (e.button !== 0) return;
    const tgt = e.target as HTMLElement;
    // 让位：reorder handle（pointer 排序）+ 交互元素（按钮/输入）
    if (tgt.closest(".drag-handle-inline, button, a, input, [role=button]")) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setGroupDrag({ pid: p.id, pname: p.name, x: e.clientX, y: e.clientY });
  };
  const onStandaloneGroupPointerMove = (e: React.PointerEvent) => {
    setGroupDrag(d => d ? { ...d, x: e.clientX, y: e.clientY } : d);
    if (!groupDrag) return;
    clearGroupHighlight();
    const found = findGroupAt(e.clientX, e.clientY);
    if (found) {
      found.el.style.outline = "2px solid var(--accent)";
      found.el.style.outlineOffset = "2px";
      groupHighlightEl.current = found.el;
    }
  };
  const onStandaloneGroupPointerUp = (e: React.PointerEvent) => {
    if (!groupDrag) return;
    const pid = groupDrag.pid;
    const found = findGroupAt(e.clientX, e.clientY);
    clearGroupHighlight();
    setGroupDrag(null);
    if (!found) return;
    groupDetailApi.movePlatform(pid, 0, found.gid)
      .then(() => {
        setToast({ text: "已加入分组", ok: true });
        // 拖到分组只改归属：平台行本身不变，仅刷 groupDetails 重建 membership（卡片即移到目标组），
        // 无需整页 load()。保留事件广播供 GroupsEmbedded 等跨组件同步。
        handleGroupsChanged();
        window.dispatchEvent(new Event("aidog-groups-changed"));
      })
      .catch(err => setToast({ text: `加入分组失败: ${err}`, ok: false }));
  };

  // ════════════ QUOTA SUBSYSTEM ════════════
  const quota = usePlatformQuota(t);

  // ════════════ MEMBERSHIP / GROUPS ════════════
  const [groupDetails, setGroupDetails] = useState<GroupDetail[]>([]);
  const [platformMembership, setPlatformMembership] = useState<Map<number, string[]>>(new Map());
  /** 纯函数：从 groupDetails 构建 platformId → groupNames[] */
  function buildMembership(gds: GroupDetail[]): Map<number, string[]> {
    const m = new Map<number, string[]>();
    for (const g of gds) {
      for (const gp of g.platforms) {
        const arr = m.get(gp.platform.id) ?? [];
        arr.push(g.group.name);
        m.set(gp.platform.id, arr);
      }
    }
    return m;
  }
  /** 分组变更：refetch groupDetails，effect 自动重建 membership */
  const handleGroupsChanged = async () => {
    try {
      setGroupDetails(await groupDetailApi.list());
    } catch { /* ignore */ }
  };

  /** 全量 refetch platforms state：删平台（Groups 页 confirmDeletePlatform）后由父级 onPlatformDeleted
   *  回调触发。++epoch 让派生层（membership/standalonePlatforms）跟随重算，对齐 load() 现有写链。
   *  复用 load() 的 epoch 守卫语义：自增 epoch 后整列表覆盖，防在途乐观写回弹。 */
  const refreshPlatforms = async () => {
    platformsEpochRef.current++;
    try {
      const list = (await platformApi.list()) || [];
      setPlatforms(list);
    } catch (e) {
      console.error("refreshPlatforms failed", e);
    }
  };

  // ════════════ SHARED STATE (toast / breaker 默认 / 搜索 / consumedEditPid ref) ════════════
  // 全局 toast：list 态（拖入分组/删除/测试）+ form 态（保存/批量）共用，故留本 hook。
  const [toast, setToast] = useState<{ text: string; ok: boolean } | null>(null);
  // 全局调度+熔断默认（用于展示「继承默认 N」），本 hook effect 异步拉取，经 listDeps 注入 form hook。
  const [breakerDefaults, setBreakerDefaults] = useState<SchedulingBreakerSettings | null>(null);
  // 平台管理页关键词搜索（纯前端 filter，按 name/base_url/协议拼音匹配）— 列表态过滤，留本 hook。
  const [searchQuery, setSearchQuery] = useState("");
  // 外部导航上下文（如分组展开区点「编辑」→ onNavigate("platforms",{platformId})）打开对应平台编辑页。
  // resetForm 复位此 ref 防二次编辑短路；声明前置以供 usePlatformForm listDeps 引用。
  const consumedEditPidRef = useRef<number | null>(null);

  // ════════════ FORM SUBSYSTEM (state + handlers 抽到 usePlatformForm) ════════════
  // ponytail: form state 与 form handlers 内聚，独立 hook；list 侧依赖（platforms/setPlatforms/
  //   quota/handleGroupsChanged 等）经 listDeps 注入，保持闭包共享正确。
  const form = usePlatformForm({
    t, platforms, setPlatforms, platformsEpochRef, quota,
    groupDetails, setGroupDetails, handleGroupsChanged, groupsReloadRef,
    setToast, breakerDefaults, setUsageMap, setLastTestMap,
    onNavigate, consumedEditPidRef,
  });

  // ════════════ LOAD / REFRESH ════════════
  const load = async () => {
    setLoading(true);
    const epoch = platformsEpochRef.current;
    let list: Platform[] = [];
    try {
      list = (await platformApi.list()) || [];
    } catch (e) { console.error(e); }
    // 在途期间发生本地乐观写（删除/保存/清理）则放弃整列表覆盖，避免晚到 resolve 回弹。
    if (epoch !== platformsEpochRef.current) { setLoading(false); return; }

    // quota 调度状态必须在 setPlatforms（→ DOM 提交 → IntersectionObserver 初次回调）之前同步就绪，
    //     否则 observer 初次 fire 时 quotaWantMapRef 仍为空 → enqueueQuota 早退 → 首屏卡片 quota 永不查
    //     （cards 已 intersecting，无后续 isIntersecting 跳变可再触发）。这是「余额/coding plan 全不展示」根因。
    quota.resetForLoad(list);

    setPlatforms(list);
    // 初始化展开态：从 platform.extra._ui_expand_plat 回灌（跨会话持久化）。
    setExpandedIds(new Set(list.filter(p => readExtraExpanded(p.extra)).map(p => p.id)));
    // 平台列表到手即渲染，余额/用量改后台渐进填充，禁止外部 quota HTTP 阻塞整页
    setLoading(false);

    // 渐进档：usage stats 单次批量（GROUP BY platform_id，含 platform_id=0 回溯），替换逐平台 N+1。
    setUsageLoading(true);
    try {
      const all = await platformApi.usageStatsAll();
      setUsageMap(all || {});
    } catch { /* ignore */ }
    finally {
      setUsageLoading(false);
    }

    // 平台「最近一次测试」徽章数据：并行拉取每平台最新 test 日志，有值才填（null 不填 = 不渲染徽章）
    Promise.all(list.map(p => platformApi.lastTestResult(p.id).catch(() => null)))
      .then(results => {
        const map: Record<number, LastTestResult> = {};
        results.forEach((r, i) => {
          if (r && list[i]) map[list[i].id] = r;
        });
        setLastTestMap(map);
      })
      .catch(() => { /* ignore */ });
  };

  /** 轻量刷新：按 id 局部 merge 派生统计字段（est_balance/est_coding_plan 等）+ usage stats 批量，
   *  不拉 quota HTTP、不整列表替换。高频被动触发（proxy log 订阅），整列表替换会打断 memo / 拖拽态
   *  并与乐观操作竞争回弹，故改为：仅更新已存在平台的字段，新增/删除的行交由显式写操作或 load() 处理。 */
  const refreshStats = async () => {
    const epoch = platformsEpochRef.current;
    try {
      const list = await platformApi.list();
      if (list && epoch === platformsEpochRef.current) {
        const byId = new Map(list.map(p => [p.id, p]));
        setPlatforms(prev => {
          let changed = false;
          const next = prev.map(p => {
            const fresh = byId.get(p.id);
            // 只 merge 后台派生的统计字段，保留前端排序/乐观态；字段相同则保引用（利于 memo）。
            if (!fresh) return p;
            if (
              fresh.est_balance_remaining === p.est_balance_remaining &&
              fresh.est_coding_plan === p.est_coding_plan &&
              fresh.last_real_query_at === p.last_real_query_at &&
              fresh.estimate_count === p.estimate_count &&
              fresh.last_error === p.last_error &&
              fresh.last_error_at === p.last_error_at
            ) return p;
            changed = true;
            return {
              ...p,
              est_balance_remaining: fresh.est_balance_remaining,
              est_coding_plan: fresh.est_coding_plan,
              last_real_query_at: fresh.last_real_query_at,
              estimate_count: fresh.estimate_count,
              last_error: fresh.last_error,
              last_error_at: fresh.last_error_at,
            };
          });
          return changed ? next : prev;
        });
      }
      const all = await platformApi.usageStatsAll();
      setUsageMap(all || {});
    } catch { /* ignore */ }
  };

  const handleDelete = async (id: number) => {
    // 删平台后端会清理 group_platform 关联并可能删孤儿 auto 组，
    // 故须刷新 groupDetails（重建 membership chips + 已分组/未分组归属），仅刷平台列表会留陈旧分组态。
    // 局部刷新：乐观从列表按 id 移除（不整页 load），失败回滚。
    let removed: Platform | undefined;
    let removedIndex = -1;
    platformsEpochRef.current++;
    setPlatforms(prev => {
      removedIndex = prev.findIndex(x => x.id === id);
      if (removedIndex >= 0) removed = prev[removedIndex];
      return prev.filter(x => x.id !== id);
    });
    try {
      await platformApi.delete(id);
      handleGroupsChanged();
      window.dispatchEvent(new Event("aidog-groups-changed"));
    } catch (e) {
      console.error(e);
      // 回滚：把被删平台插回原位。
      if (removed) {
        const r = removed; const idx = removedIndex;
        setPlatforms(prev => {
          if (prev.some(x => x.id === r.id)) return prev;
          const next = [...prev];
          next.splice(idx >= 0 && idx <= next.length ? idx : next.length, 0, r);
          return next;
        });
      }
      setToast({ text: `${t("platform.deleteFail", "删除失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  const handleToggle = async (p: Platform) => {
    // 三态切换：enabled → disabled；disabled / auto_disabled → enabled（恢复并清退避）。
    const nextStatus: PlatformStatus = p.status === "enabled" ? "disabled" : "enabled";
    // 乐观更新：立即本地置换该平台 status，UI 即时响应、不调 load() 全量重拉（避免整页 loading 闪烁）。
    // status 切换不改分组归属（membership 由 groupDetails 决定），故无需广播 aidog-groups-changed。
    setPlatforms(prev => prev.map(x =>
      x.id === p.id ? { ...x, status: nextStatus, enabled: nextStatus === "enabled" } : x));
    try {
      const updated = await platformApi.update({ id: p.id, status: nextStatus });
      // 用后端返回值校正单个 item（含清退避后的派生字段），仍不动其他平台、不重拉列表。
      setPlatforms(prev => prev.map(x => x.id === p.id ? updated : x));
    } catch (e) {
      console.error(e);
      // 失败回滚该 item 到原状态 + 报错。
      setPlatforms(prev => prev.map(x => x.id === p.id ? p : x));
      setToast({ text: `${p.name}: ${t("platform.toggleFail", "切换失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  const handleQuickTest = async (p: Platform) => {
    setTestingId(p.id);
    let success = false;
    try {
      const defaultModel = p.models.default || p.available_models[0] || "";
      const r = await modelTestApi.test({ platform_id: p.id, model: defaultModel });
      success = r.success;
      setTestResults(prev => ({ ...prev, [p.id]: r.success ? "ok" : "fail" }));
      setToast({ text: r.success
        ? `${p.name}: ${t("platform.testOk", "测试成功")}${r.duration_ms > 0 ? ` (${r.duration_ms}ms)` : ""}`
        : `${p.name}: ${r.error || t("platform.testFail", "测试失败")}`,
        ok: r.success });
    } catch (err: any) {
      setTestResults(prev => ({ ...prev, [p.id]: "fail" }));
      setToast({ text: `${p.name}: ${err?.message || t("platform.testFail", "测试失败")}`, ok: false });
    }
    setTestingId(null);
    setTimeout(() => setToast(null), 3000);
    // 派发全局事件：跨页（Groups 批量测 / ModelTestPanel 自定义）跑测后切到本页，本页卡片徽章 + health 据此即时刷新
    window.dispatchEvent(new CustomEvent("aidog-platform-test-completed", { detail: { platformId: p.id, success } }));
  };

  // 拉取某平台最近一次 test 日志，刷新 lastTestMap 对应项（供 aidog-platform-test-completed 监听后调用）
  const refreshLastTest = useCallback(async (platformId: number) => {
    try {
      const r = await platformApi.lastTestResult(platformId);
      setLastTestMap(prev => {
        const next = { ...prev };
        if (r) next[platformId] = r; else delete next[platformId];
        return next;
      });
    } catch { /* ignore */ }
  }, []);

  /** 分享平台：拉取可分享配置对象（含明文 api_key）→ 打开 ShareModal（弹窗内自动复制 + 格式切换）。 */
  const handleShare = async (p: Platform) => {
    try {
      const share = await platformApi.shareExport(p.id);
      form.setShareData({ share, name: p.name });
    } catch (err) {
      console.error("platform share export failed", err);
      setToast({ text: `${p.name}: ${t("platform.share.exportFail", "生成分享内容失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  /** 清理失效平台（自动禁用态）：永久删除，乐观从列表移除，失败不动（后端事务回滚保证一致性）。 */
  const handlePurgeDisabled = async () => {
    if (!window.confirm(t("platform.purgeDisabledConfirm", "将永久删除全库失效(自动禁用)平台，不可恢复，确定？"))) return;
    try {
      const r = await platformApi.purgeDisabled();
      if (r.deletedIds.length === 0) {
        setToast({ text: t("platform.purgeDisabledNone", "暂无失效平台"), ok: true });
      } else {
        setToast({ text: t("platform.purgeDisabledDone", "已删除 {{count}} 个失效平台", { count: r.deletedIds.length }), ok: true });
      }
      setTimeout(() => setToast(null), 3000);
      // 局部刷新：按 deletedIds 批量移除被永久删除的平台（不整页 load）；
      // unassignedIds（仅移除分组关联，平台行保留）的归属变化由 handleGroupsChanged 重建 membership。
      if (r.deletedIds.length > 0) {
        const del = new Set(r.deletedIds);
        platformsEpochRef.current++;
        setPlatforms(prev => prev.filter(x => !del.has(x.id)));
      }
      handleGroupsChanged();
      groupsReloadRef.current?.();
    } catch (err) {
      setToast({ text: `${t("platform.purgeDisabled", "清理失效平台")}: ${err}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  // ════════════ DERIVED (standalone + counts) ════════════
  // 未归属任何分组的平台（主列表独立展示）；已分组平台只在 GroupsEmbedded 内展示，避免重复。
  const standalonePlatforms = useMemo(
    () => platforms
      .filter(p => !platformMembership.has(p.id))
      .filter(p => {
        const q = searchQuery.trim();
        if (!q) return true;
        return pinyinMatch(q, p.name)
          || pinyinMatch(q, p.base_url)
          || pinyinMatch(q, p.platform_type);
      }),
    [platforms, platformMembership, searchQuery],
  );
  // 列表头部「启用 / 总数」派生值：仅随 platforms 变化，避免每次轮询/拖拽重渲染时重扫全列表
  const enabledCount = useMemo(() => platforms.filter(p => p.enabled).length, [platforms]);
  // 页头徽章计数：优先用 GroupsEmbedded 渐进回传值（随各组平台逐组流入增量更新），
  // 回退本页自身 platforms 派生值（progressiveCount 尚未回传 / 被重置时）。
  const headerActive = progressiveCount ? progressiveCount.active : enabledCount;
  const headerTotal = progressiveCount ? progressiveCount.total : platforms.length;

  // ════════════ EFFECTS ════════════
  useEffect(() => { load(); }, []);

  // aidog://platform/import?data=<base64> deep-link 导入入口。
  const openDeepLinkImport = useCallback((data: string) => {
    if (!data) return;
    // SmartPasteModal 挂在 `if (showForm)` 分支内，需先开 form 再开 paste 弹窗。
    // applyPaste(fullShare) 路径整体覆盖所有字段（setEditing(null) 等），故不调 resetForm。
    form.setPasteInitialText(data);
    form.setShowForm(true);
    form.setShowPaste(true);
  }, []);
  useEffect(() => {
    const w = window as unknown as { __aidogDeepLink?: Record<string, { action: string; data: string }> };
    const cached = w.__aidogDeepLink?.platform;
    if (cached?.data) {
      delete w.__aidogDeepLink!.platform; // 消费一次防重复
      openDeepLinkImport(cached.data);
    }
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ action: string; data: string }>).detail;
      if (detail?.data) {
        // 热路径（本页已 mount）也清缓存，否则离开再回（key={effectiveNav} 重挂载）会重放。
        delete w.__aidogDeepLink!.platform;
        openDeepLinkImport(detail.data);
      }
    };
    window.addEventListener("aidog:platform", handler);
    return () => window.removeEventListener("aidog:platform", handler);
  }, [openDeepLinkImport]);

  // 可视区优先 quota 调度：IntersectionObserver 观察每张卡片（data-platform-id），
  //    进入视口即入队（enqueueQuota 去重 + 池控并发）；滚动到更多平台时触发其余。
  useEffect(() => {
    if (platforms.length === 0) return;
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const idAttr = (entry.target as HTMLElement).dataset.platformId;
        if (!idAttr) continue;
        const pid = Number(idAttr);
        const p = quota.quotaWantMapRef.current.get(pid);
        if (p) quota.enqueueQuota(p);
      }
    }, { root: null, rootMargin: "200px", threshold: 0 });
    const el = platListRef.current;
    if (el) el.querySelectorAll<HTMLElement>("[data-platform-id]").forEach(card => observer.observe(card));
    return () => { observer.disconnect(); };
  }, [platforms, quota]);

  // 外部导航上下文（如分组展开区点「编辑」→ onNavigate("platforms",{platformId})）打开对应平台编辑页。
  // ponytail: consumedEditPidRef 已在上方 shared state 节声明（供 usePlatformForm 的 resetForm 复位）。
  useEffect(() => {
    const pid = initialFilter?.platformId;
    if (!pid || consumedEditPidRef.current === pid) return;
    const target = platforms.find(p => p.id === pid);
    if (!target) return;  // 列表尚未加载到该平台，待 platforms 更新后重试
    consumedEditPidRef.current = pid;
    if (initialFilter?.duplicate) form.handleDuplicate(target);
    else form.handleEdit(target);
  }, [initialFilter?.platformId, initialFilter?.duplicate, platforms]);

  // 分组列表（multi-select 数据源 + 编辑态反查手动组归属 + 平台归属映射）。本地查询，失败不阻断编辑。
  useEffect(() => {
    groupDetailApi.list().then(setGroupDetails).catch(() => {});
  }, []);

  // groupDetails 变化时重建 membership（初始加载 + 所有 setGroupDetails 路径都覆盖）
  useEffect(() => { setPlatformMembership(buildMembership(groupDetails)); }, [groupDetails]);

  // 全局调度+熔断默认（展示「继承默认 N」用），读失败不阻断编辑。
  useEffect(() => {
    (async () => {
      try {
        setBreakerDefaults(await schedulingApi.getSettings());
      } catch (e) {
        console.error("get scheduling settings failed", e);
      }
    })();
  }, []);

  // 请求完成后轻量刷新统计（仅本地 DB 查询，不拉 quota HTTP）
  useEffect(() => onProxyLogUpdated(() => { refreshStats(); }), []);

  // 监听全局测试完成事件：单卡刷新「最近测试」徽章 + 写 testResults（驱动 health 走 manual 分支）
  useEffect(() => {
    const handler = (e: Event) => {
      const ce = e as CustomEvent<{ platformId: number; success?: boolean }>;
      const pid = ce.detail?.platformId;
      if (pid == null) return;
      refreshLastTest(pid);
      if (ce.detail.success != null) {
        setTestResults(prev => ({ ...prev, [pid]: ce.detail.success ? "ok" : "fail" }));
      }
    };
    window.addEventListener("aidog-platform-test-completed", handler);
    return () => window.removeEventListener("aidog-platform-test-completed", handler);
  }, [refreshLastTest]);

  return {
    t, onNavigate, initialFilter, groupsReloadRef,
    platforms, setPlatforms, platformsEpochRef,
    usageMap, setUsageMap, usageLoading,
    testResults, setTestResults, lastTestMap, setLastTestMap,
    testingId, setTestingId, loading,
    progressiveCount, setProgressiveCount,
    faviconFailed, setFaviconFailed, expandedIds, toggleExpanded,
    platDrag, platListRef, handlePlatPointerDown, handlePlatPointerMove, handlePlatPointerUp,
    groupDrag, onStandaloneGroupPointerDown, onStandaloneGroupPointerMove, onStandaloneGroupPointerUp,
    quota,
    platformMembership, groupDetails, setGroupDetails, handleGroupsChanged, refreshPlatforms,
    standalonePlatforms, searchQuery, setSearchQuery,
    enabledCount, headerActive, headerTotal,
    toast, setToast,
    // ── form 子系统（state + handlers 全部来自 usePlatformForm，含 breakerDefaults）──
    ...form,
    // ── list 侧 handlers ──
    load, refreshStats,
    handleDelete, handleToggle, handleQuickTest, handleShare,
    handlePurgeDisabled,
    getPrimaryBaseUrl,
  };
}
