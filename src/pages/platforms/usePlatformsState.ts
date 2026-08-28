// usePlatformsState — Platforms 主组件的 state + handlers 编排层。
// ponytail: 收编 Platforms 主组件除 quota 子系统（usePlatformQuota）+ form 子系统（usePlatformForm）
//   外的全部 list/drag/CRUD/effect 逻辑。form state + form handlers 已抽到 usePlatformForm.ts
//   （经 listDeps 注入 list 侧依赖保持闭包共享）。本 hook 负责 list 态 + 派生 + effects + return 组装。
//
// 子组件消费：PlatformEditForm（编辑态）+ PlatformListView（列表态）通过 props 拿本 hook 返回值。
//
// ── platform mutation 一致性规则（arch-deepen/c6-event-bus） ──────────────────────────
// 影响分组归属且改平台行的写路径必须三连：handleGroupsChanged() + groupsReloadRef.current?.()
//   + window.dispatchEvent(new Event("aidog-groups-changed"))。
//   - handleGroupsChanged: 刷新本 hook 的 groupDetails/membership（chips + 已分组/未分组派生）
//   - groupsReloadRef: 命令式触发 GroupsEmbedded 重建（其 platforms state 独立，父级 setPlatforms 触达不到）
//   - aidog-groups-changed 事件: 跨组件广播（其他监听者按需响应）
// 对齐位置：handleDelete / handlePurgeDisabled（本文件）+ handleSave
//   （usePlatformForm.ts）+ runBatchCreateFromPaste（platformPasteApply.ts）。
//
// 故意漏 reloadRef 的特例：拖入分组（onStandaloneGroupPointerUp）—平台行不变，仅 setGroupDetails
//   + 事件广播即足够，跳过 reloadRef（见 :343-344 注释）。
//
// 不发 groups-changed 的路径：handleToggle（status 切换不改归属）/ handleQuickTest（仅测试信号）。
//
// 正交信号（不混入三连）: aidog-platform-test-completed（单卡测试完成）/ aidog:platform（deep-link
//   导入）—各自有独立 lifecycle 与监听者，不与 membership mutation 混。
// ────────────────────────────────────────────────────────────────────────────────────
import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  platformApi, modelTestApi, groupDetailApi, schedulingApi,
  onProxyLogUpdated,
  type Platform, type PlatformStatus, type Protocol, type PlatformEndpoint,
  type PlatformUsageStats, type LastTestResult,
  type SchedulingBreakerSettings, type GroupDetail,
} from "../../services/api";
import { platformMatchesQuery } from "../../domains/groups";
import { getProtocolSearchTermsMap } from "../../domains/platforms/defaults";
import { usePlatformQuota, getPrimaryBaseUrl } from "./usePlatformQuota";
import { usePlatformForm, type PlatformFormState } from "./usePlatformForm";
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

// ════════════ STATE SLICES (arch-deepen/c7-god-surface reducer 化) ════════════
// ponytail: 60 字段 god interface 归一为 4 子系统 slice + params echo。
//   - list slice: list state (platforms/usage/test/loading/derived/membership) + list handlers
//     (load/refresh/delete/toggle/test/share/purge/groups/refresh/remove/toggleExpand)
//   - drag slice: 拖拽 reorder + 跨区域 group drag (pointer events + refs)，imperative 路径保留 hook 形态
//   - form slice: 来自 usePlatformForm 的全表单 state (~30 字段，整包透传)
//   - quota slice: 来自 usePlatformQuota 的余额调度子系统 (整包透传)
// 内部 useState 集群不强制 useReducer（多数 setter 经 usePlatformForm listDeps 注入，reader 反增 dispatch
//   桥接成本）；本归一仅 reshape 接口 surface，state model 与所有调用链保持原语义（测试零回归）。

/** list 子系统 slice：列表 state + 派生 + list 侧 handlers。 */
export interface PlatformsListSlice {
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
  // ── membership / groups ──
  platformMembership: Map<number, string[]>;
  groupDetails: GroupDetail[];
  setGroupDetails: React.Dispatch<React.SetStateAction<GroupDetail[]>>;
  handleGroupsChanged: () => Promise<void>;
  /** 平台被删后全量 refetch platforms state（独立信号，与 onGroupsChanged 分组刷新分离）。
   *  - 触发点：PlatformEditForm CPA apply 创建场景（保存后整列表 refetch，语义合理）。
   *  - Groups 删平台不再用本方法（改 removePlatformsByIds 局部移除，消除整页刷新体感）。 */
  refreshPlatforms: () => Promise<void>;
  /** 平台被删后局部移除（按 id filter，不调 API，epoch++ 先于 setPlatforms 触发派生层重算）。
   *  调用方必须已先成功调 delete API（失败不误移）。Groups 页 confirmDeletePlatform /
   *  confirmBatchDelete 用本方法替代 refreshPlatforms 全量 refetch。 */
  removePlatformsByIds: (ids: number[]) => void;
  // ── standalone (未分组 + 搜索) ──
  standalonePlatforms: Platform[];
  searchQuery: string;
  setSearchQuery: React.Dispatch<React.SetStateAction<string>>;
  // ── derived counts ──
  enabledCount: number;
  headerActive: number;
  headerTotal: number;
  // ── toast (list 态 + form 态共用) ──
  toast: { text: string; ok: boolean } | null;
  setToast: React.Dispatch<React.SetStateAction<{ text: string; ok: boolean } | null>>;
  // ── list handlers ──
  load: () => Promise<void>;
  refreshStats: () => Promise<void>;
  handleDelete: (id: number) => Promise<void>;
  handleToggle: (p: Platform) => Promise<void>;
  handleQuickTest: (p: Platform) => Promise<void>;
  handleShare: (p: Platform) => Promise<void>;
  handlePurgeDisabled: () => Promise<void>;
}

/** drag 子系统 slice：列表内 reorder + 跨区域拖入分组 (pointer events + refs)。
 *  ponytail: 拖拽是高频 imperative 路径（pointermove 每帧 setState + ref 读写），reducer 反增复杂度
 *    （dispatch 桥接 + ref 透传），故保留 hook 形态整包透传，仅 reshape 接口 surface。 */
export interface PlatformsDragSlice {
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
}

export interface PlatformsState extends PlatformsStateParams {
  t: TFunction;
  // ── 四子系统 slice (取代 60 字段 god interface) ──
  list: PlatformsListSlice;
  drag: PlatformsDragSlice;
  /** 来自 usePlatformForm 的全表单 state + handlers（~30 字段整包透传，原 `...form` 展开改为 slice 命名访问）。 */
  form: PlatformFormState;
  /** 来自 usePlatformQuota 的余额/配额调度子系统（整包透传）。 */
  quota: ReturnType<typeof usePlatformQuota>;
  /** 主 URL 推导 helper（form header desc + fetch models 回退链共用；非 slice 内聚，留顶层）。 */
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
        // 拖到分组特例（见顶部「一致性规则」— 故意漏 reloadRef）：平台行本身不变，仅刷 groupDetails
        //   重建 membership（卡片即移到目标组）+ 事件广播供 GroupsEmbedded 等跨组件同步，无需整页 load()。
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
  /** 分组变更：refetch groupDetails，effect 自动重建 membership。
   *  本函数是 platform mutation 三连之一（见顶部「一致性规则」）；各 mutation 点调用方负责按序补齐
   *  groupsReloadRef.current?.() + window.dispatchEvent(aidog-groups-changed)。 */
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

  /** 局部移除：按 id filter，不调 API（调用方已调），epoch++ 先于 setPlatforms 让派生层重算。
   *  复用 handleDelete:492 的乐观移除模式，但删除 API 调用交调用方（Groups 已调）。 */
  const removePlatformsByIds = useCallback((ids: number[]) => {
    if (ids.length === 0) return;
    platformsEpochRef.current++;
    const idSet = new Set(ids);
    setPlatforms(prev => prev.filter(x => !idSet.has(x.id)));
  }, []);

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
      // platform mutation 三连（见顶部「一致性规则」）：对齐 handleSave /
      //   runBatchCreateFromPaste / handlePurgeDisabled。删平台会 cascade 清 group_platform 关联，
      //   GroupsEmbedded 分组卡内的该平台行必须由专用 reload 移除，仅靠父级 setPlatforms(filter) 无法触达
      //   （GroupsEmbedded 渲染门控在其自身 platforms state）。
      handleGroupsChanged();
      groupsReloadRef.current?.();
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
    // status 切换不改分组归属（membership 由 groupDetails 决定），故豁免 platform mutation 三连
    //   （见顶部「一致性规则」— 不发 aidog-groups-changed / 不调 groupsReloadRef / 不调 handleGroupsChanged）。
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

  /** 清理失效平台（自动禁用态）：永久删除，乐观从列表移除，失败不动（后端事务回滚保证一致性）。
   *  ponytail: 确认对话框由调用方 (PlatformListView AlertDialog) 处理，本函数仅执行 invoke。 */
  const handlePurgeDisabled = async () => {
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
      // platform mutation 三连（见顶部「一致性规则」）：对齐 handleSave /
      //   runBatchCreateFromPaste / handleDelete。purge 会移除分组关联（unassignedIds）+ 永久删除部分平台
      //   （deletedIds），跨组件需感知成员变更。
      handleGroupsChanged();
      groupsReloadRef.current?.();
      window.dispatchEvent(new Event("aidog-groups-changed"));
    } catch (err) {
      setToast({ text: `${t("platform.purgeDisabled", "清理失效平台")}: ${err}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  };

  // 协议搜索词（registry name 全 locale + keywords）：挂载时拉一次，搜索跨语言匹配用
  const [protocolTerms, setProtocolTerms] = useState<Partial<Record<string, string[]>>>({});
  useEffect(() => {
    let cancelled = false;
    getProtocolSearchTermsMap().then(m => { if (!cancelled) setProtocolTerms(m); }).catch(console.error);
    return () => { cancelled = true; };
  }, []);

  // ════════════ DERIVED (standalone + counts) ════════════
  // 未归属任何分组的平台（主列表独立展示）；已分组平台只在 GroupsEmbedded 内展示，避免重复。
  const standalonePlatforms = useMemo(
    () => platforms
      .filter(p => !platformMembership.has(p.id))
      .filter(p => {
        const q = searchQuery.trim();
        if (!q) return true;
        return platformMatchesQuery(p, q, protocolTerms);
      }),
    [platforms, platformMembership, searchQuery, protocolTerms],
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

  // ════════════ SLICE AGGREGATION (arch-deepen/c7-god-surface) ════════════
  // ponytail: 60 字段 god interface → 4 子系统 slice + params echo (顶层 8 字段)。
  //   内部 state/handler 实现零改，仅 reshape 返回 surface；所有 setter 经 listDeps/form 注入路径保留。
  const listSlice: PlatformsListSlice = {
    platforms, setPlatforms, platformsEpochRef,
    usageMap, setUsageMap, usageLoading,
    testResults, setTestResults, lastTestMap, setLastTestMap,
    testingId, setTestingId, loading,
    progressiveCount, setProgressiveCount,
    faviconFailed, setFaviconFailed, expandedIds, toggleExpanded,
    platformMembership, groupDetails, setGroupDetails,
    handleGroupsChanged, refreshPlatforms, removePlatformsByIds,
    standalonePlatforms, searchQuery, setSearchQuery,
    enabledCount, headerActive, headerTotal,
    toast, setToast,
    load, refreshStats,
    handleDelete, handleToggle, handleQuickTest, handleShare, handlePurgeDisabled,
  };
  const dragSlice: PlatformsDragSlice = {
    platDrag, platListRef,
    handlePlatPointerDown, handlePlatPointerMove, handlePlatPointerUp,
    groupDrag,
    onStandaloneGroupPointerDown, onStandaloneGroupPointerMove, onStandaloneGroupPointerUp,
  };

  return {
    t, onNavigate, initialFilter, groupsReloadRef,
    list: listSlice,
    drag: dragSlice,
    form,
    quota,
    getPrimaryBaseUrl,
  };
}
