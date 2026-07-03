import { useEffect, useRef, useState } from "react";
import type { GroupDetail, Platform, PlatformUsageStats } from "../../services/api";
import {
  groupDetailApi, groupUsageApi, platformApi, proxyApi, onProxyLogUpdated,
} from "../../services/api";
import { upsertPlatformInto } from "../../domains/groups";

/**
 * 拉取每个 group 的使用统计 + 余额。
 * - usage stats：按 proxy_log.group_key 聚合（`groupUsageApi.statsAll` 单次批量），只含本分组请求，共享平台不重复计入。
 * - balance：关联 platforms 的 est_balance_remaining 求和（平台级属性，无 per-group 概念，维持现状）。
 * load 与 refreshStats 共用，避免两处求和逻辑重复。
 */
export async function fetchGroupStats(
  details: GroupDetail[],
  platforms: Platform[],
): Promise<{ statsMap: Record<string, PlatformUsageStats>; balanceMap: Record<number, number> }> {
  const platById = new Map(platforms.map(pp => [pp.id, pp]));
  const statsMap: Record<string, PlatformUsageStats> = {};
  const balanceMap: Record<number, number> = {};
  // usage stats：单次批量 invoke（后端 GROUP BY group_key），消除逐 group N+1 往返。
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

export interface UseGroupDataArgs {
  onCountChange?: (counts: { total: number; active: number } | null) => void;
}

/**
 * 分组数据加载：
 *   ① 一次批量 `group_detail_list_paged`（无 JOIN）+ `platform_list`（全量）并行；
 *   ② 先渲分组骨架，触底加载 sentinel 触发下一页；
 *   ③ 统计/余额按已加载组重算。
 * mount 跑 load()；reloadRef / aidog-groups-changed 由调用方按需触发 load()。
 */
export function useGroupData({ onCountChange }: UseGroupDataArgs = {}) {
  const [details, setDetails] = useState<GroupDetail[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [groupStats, setGroupStats] = useState<Record<string, PlatformUsageStats>>({});
  // 聚合余额：关联 platforms 的 est_balance_remaining 求和（platformApi.list 已带，无额外 HTTP）。
  const [groupBalance, setGroupBalance] = useState<Record<number, number>>({});
  const [loading, setLoading] = useState(true);
  // 代理端口（proxy_get_settings），构造页面级 base_url；取失败兜底 7890。
  const [proxyPort, setProxyPort] = useState(7890);
  const proxyBaseUrl = `http://127.0.0.1:${proxyPort}/proxy`;

  // 渐进加载序号守卫：每次 load() 自增，异步阶段回调前比对，丢弃陈旧轮次（reload/StrictMode 双跑）的迟到 setState。
  const loadSeqRef = useRef(0);

  // ── 触底加载（J3：反转 H6 单 JOIN 全量批量，改前端分页无限滚动）──
  const PAGE_SIZE = 12;
  const nextOffsetRef = useRef(0);
  const allPlatformsRef = useRef<Platform[]>([]);
  const loadedDetailsRef = useRef<GroupDetail[]>([]);
  const [hasMore, setHasMore] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const loadingMoreRef = useRef(false);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  /** 计数回传（跨组/未分组去重）：基于「全量平台快照」算 total/active，与分页进度解耦。 */
  const reportCount = () => {
    const all = allPlatformsRef.current;
    let active = 0;
    for (const p of all) if (p.enabled) active++;
    onCountChange?.({ total: all.length, active });
  };

  /** 已拉全部组 detail + 全量平台 → 重算统计/余额（增量加载后复用，无额外组 invoke 往返）。 */
  const recomputeStats = async (seqAlive: () => boolean) => {
    const { statsMap, balanceMap } = await fetchGroupStats(loadedDetailsRef.current, allPlatformsRef.current);
    if (!seqAlive()) return;
    setGroupStats(statsMap);
    setGroupBalance(balanceMap);
  };

  /** 轻量刷新：刷新全量平台快照（含 est_balance_remaining）+ 按已加载组重算 usage stats / 余额聚合，
   *  不拉 quota HTTP、不重拉组（统计基于已触底加载的 loadedDetailsRef，分页一致）。 */
  const refreshStats = async () => {
    try {
      const p = (await platformApi.list()) || [];
      allPlatformsRef.current = p;
      const { statsMap, balanceMap } = await fetchGroupStats(loadedDetailsRef.current, p);
      setGroupStats(statsMap);
      setGroupBalance(balanceMap);
    } catch { /* ignore */ }
  };

  /**
   * 触底加载下一页组 detail（后端 group_detail_list_paged，无 JOIN）。
   * seq 守卫：load() 自增 loadSeqRef，分页迟到回调比对丢弃陈旧轮次。
   */
  const loadMore = async () => {
    if (loadingMoreRef.current) return;
    const seq = loadSeqRef.current;
    const alive = () => seq === loadSeqRef.current;
    loadingMoreRef.current = true;
    setLoadingMore(true);
    try {
      const offset = nextOffsetRef.current;
      const page = (await groupDetailApi.listPaged(offset, PAGE_SIZE)) || [];
      if (!alive()) return;
      nextOffsetRef.current = offset + PAGE_SIZE;
      if (page.length < PAGE_SIZE) setHasMore(false);
      if (page.length === 0) return;

      const filled: GroupDetail[] = page.map(d => ({
        group: d.group,
        platforms: d.platforms || [],
        model_mappings: d.model_mappings || d.group.model_mappings || [],
      }));
      loadedDetailsRef.current = [...loadedDetailsRef.current, ...filled];
      setDetails(prev => [...prev, ...filled]);
      setPlatforms(prev => {
        let next = prev;
        for (const d of filled) for (const gp of d.platforms) next = upsertPlatformInto(next, gp.platform);
        return next;
      });
      reportCount();
      await recomputeStats(alive);
    } catch (e) {
      console.error(e);
    } finally {
      loadingMoreRef.current = false;
      if (alive()) setLoadingMore(false);
    }
  };

  /**
   * 全量重载（mount / 结构变化）：重置分页游标 + 全量平台快照，拉第一页组 detail（触底加载首屏）。
   * 后续页由 sentinel IntersectionObserver 经 loadMore 拉取。组列表去 JOIN → 分页（J3 反转 H6）。
   */
  const load = async () => {
    const seq = ++loadSeqRef.current;
    const alive = () => seq === loadSeqRef.current;
    setLoading(true);
    onCountChange?.({ total: 0, active: 0 });
    nextOffsetRef.current = 0;
    loadedDetailsRef.current = [];
    loadingMoreRef.current = false;
    setHasMore(true);
    setDetails([]);
    setPlatforms([]);
    try {
      const allPlatforms: Platform[] = (await platformApi.list()) || [];
      if (!alive()) return;
      allPlatformsRef.current = allPlatforms;
      setPlatforms(prev => {
        let next = prev;
        for (const plat of allPlatforms) next = upsertPlatformInto(next, plat);
        return next;
      });
      reportCount();
      setLoading(false);
      await loadMore();
    } catch (e) {
      console.error(e);
      if (alive()) setLoading(false);
    }
  };

  /**
   * 单组就地刷新：只重拉该组 detail（O(1) 一次往返），原地替换对应 GroupDetail
   * + 把该组平台 upsert 进 platforms（保留其余组卡引用稳定，避免 load() 全量重渲闪烁）。
   * 组结构已变（增删组）时回退全量 load()。
   */
  const refreshSingleGroup = async (gid: number) => {
    try {
      const d = await groupDetailApi.get(gid);
      if (!d) { load(); return; } // 该组已不存在（被删）→ 全量回退
      const filled: GroupDetail = {
        group: d.group,
        platforms: d.platforms || [],
        model_mappings: d.model_mappings || d.group.model_mappings || [],
      };
      let found = false;
      setDetails(prev => {
        const next = prev.map(x => {
          if (x.group.id !== gid) return x;
          found = true;
          return filled;
        });
        return found ? next : prev;
      });
      if (!found) { load(); return; }
      loadedDetailsRef.current = loadedDetailsRef.current.map(x => x.group.id === gid ? filled : x);
      setPlatforms(prev => {
        let next = prev;
        for (const gp of filled.platforms) next = upsertPlatformInto(next, gp.platform);
        return next;
      });
      refreshStats();
    } catch (e) {
      console.error(e);
      load();
    }
  };

  // ── mount load ──
  useEffect(() => { load(); }, []);

  // ── 触底加载 sentinel：滚到底部（含全屏视图退出后）拉下一页组 detail（J3 无限滚动）。
  useEffect(() => {
    const el = sentinelRef.current;
    if (!el || !hasMore || loading) return;
    const io = new IntersectionObserver(entries => {
      if (entries.some(e => e.isIntersecting) && !loadingMoreRef.current) {
        loadMore();
      }
    }, { rootMargin: "200px" });
    io.observe(el);
    return () => io.disconnect();
  }, [hasMore, loading, loadingMore]);

  // 取代理端口构造 base_url；失败保持兜底 7890。
  useEffect(() => {
    proxyApi.getSettings()
      .then(s => { if (s?.port) setProxyPort(s.port); })
      .catch(() => { /* 兜底 7890 */ });
  }, []);

  // 请求完成后轻量刷新统计（仅本地 DB 查询，不拉 quota HTTP）
  useEffect(() => onProxyLogUpdated(() => { refreshStats(); }), []);

  // 监听跨组件分组变更（Platforms pointer 拖入分组后通知刷新；HTML5 DnD 跨区域在 WKWebView 失效，改 pointer + window 事件）
  useEffect(() => {
    const h = () => { load(); refreshStats(); };
    window.addEventListener("aidog-groups-changed", h);
    return () => window.removeEventListener("aidog-groups-changed", h);
  }, []);

  // groupApi.reorder 在 handleReorderGroups（保留在主文件）使用 —— 这里 re-export 以便主文件复用；
  // ponytail: 不另起 hooks 文件，groupApi 已在 services/api barrel 暴露
  return {
    details, platforms, setDetails,
    groupStats, groupBalance, setGroupBalance,
    loading, loadingMore, hasMore, sentinelRef,
    proxyBaseUrl, allPlatformsRef,
    load, refreshStats, refreshSingleGroup,
  };
}
