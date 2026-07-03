// usePlatformQuota — 平台余额/配额调度子系统。
// ponytail: 从 Platforms 主组件抽出的独立 hook，封装 quota HTTP 查询的有界并发池 +
//   IntersectionObserver 入队（owner 注入）+ 三态展示状态（pending/refreshing/realId）。
//
// 边界（自包含）：仅依赖 platformApi/quotaApi + setQuota* setters + 一个 newApi 回调；
//   不持有 platforms 列表，不关心 UI/表单。owner 通过 scheduleQuotaFor(p) 为新建/编辑平台补查。
import { useCallback, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { quotaApi, type Platform, type PlatformQuota } from "../../services/api";
import { QUOTA_CONCURRENCY } from "../../domains/platforms";

/** 从 endpoints 推导主 base_url（匹配主协议，否则取第一个）。配额查询 / 主 URL 推导共用。 */
function getPrimaryBaseUrl(proto: Platform["platform_type"], eps: Platform["endpoints"] | undefined): string {
  if (!eps || eps.length === 0) return "";
  const primary = eps.find(ep => ep.protocol === proto);
  if (primary) return primary.base_url;
  return eps[0]?.base_url || "";
}

export interface UsePlatformQuotaResult {
  quotaMap: Record<number, PlatformQuota>;
  quotaRealIds: Record<number, boolean>;
  quotaRefreshing: Record<number, boolean>;
  quotaPending: Record<number, boolean>;
  quotaQueueRef: React.MutableRefObject<Platform[]>;
  quotaScheduledRef: React.MutableRefObject<Set<number>>;
  quotaPoolActiveRef: React.MutableRefObject<number>;
  quotaWantMapRef: React.MutableRefObject<Map<number, Platform>>;
  /** 该平台是否需要外部 quota 查询（mock/claude_code 无配额；无 key / 无 base_url 不可查）。 */
  platformWantsQuota: (p: Platform) => boolean;
  /** 单平台 quota 查询（成功填 quotaMap），结束后清 pending。供有界并发池 worker 调用。 */
  fetchQuotaForPlatform: (p: Platform) => Promise<void>;
  /** 入队：把平台加入 quota 调度队列（去重），并尝试启动 worker 领取。 */
  enqueueQuota: (p: Platform) => void;
  /** 局部刷新（新建/编辑平台）专用：注入 wantMap + pending 后入队，确保非 load() 路径也能查余额。 */
  scheduleQuotaFor: (p: Platform) => void;
  /** 手动刷新：阻塞 UI 显式查 quota（带 refreshing 旋转 + toast），newapi 自动回填 user_id。 */
  refreshQuota: (p: Platform, opts?: { onNewapiUserId?: (uid: string) => void; onError?: (msg: string) => void }) => Promise<void>;
  /** 重置 quota 调度状态（load() 列表到手前同步就绪）。返回 wantMap + pending 初值供 owner 提交。 */
  resetForLoad: (list: Platform[]) => { wantMap: Map<number, Platform>; pending: Record<number, boolean> };
  setQuotaMap: React.Dispatch<React.SetStateAction<Record<number, PlatformQuota>>>;
  setQuotaPending: React.Dispatch<React.SetStateAction<Record<number, boolean>>>;
  setQuotaRefreshing: React.Dispatch<React.SetStateAction<Record<number, boolean>>>;
  setQuotaRealIds: React.Dispatch<React.SetStateAction<Record<number, boolean>>>;
}

export function usePlatformQuota(t: TFunction): UsePlatformQuotaResult {
  const [quotaMap, setQuotaMap] = useState<Record<number, PlatformQuota>>({});
  // 手动刷新（真查校准）后的平台 id → 优先展示 quotaMap 真值而非预估
  const [quotaRealIds, setQuotaRealIds] = useState<Record<number, boolean>>({});
  const [quotaRefreshing, setQuotaRefreshing] = useState<Record<number, boolean>>({});
  // 延迟档 quota 待回标志：load 时对所有需查 quota 的平台置 true，HTTP 结算（成功/失败）后置 false。
  //   余额区据此显骨架而非 est 旧值，避免闪烁回填。
  const [quotaPending, setQuotaPending] = useState<Record<number, boolean>>({});
  // quota 调度：待领取队列（按可视优先顺序入队）、已调度去重集合、需查 quota 的平台快照。
  //   IntersectionObserver 决定入队时机/优先级，有界 worker pool 控并发上限。用 ref 不触发渲染。
  const quotaQueueRef = useRef<Platform[]>([]);
  const quotaScheduledRef = useRef<Set<number>>(new Set());
  const quotaPoolActiveRef = useRef(0);
  const quotaWantMapRef = useRef<Map<number, Platform>>(new Map());

  const platformWantsQuota = useCallback((p: Platform): boolean => {
    if (p.platform_type === "mock" || p.platform_type === "claude_code") return false;
    if (!p.api_key) return false;
    return !!getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []);
  }, []);

  const fetchQuotaForPlatform = useCallback(async (p: Platform) => {
    const baseUrl = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []);
    try {
      const q = p.platform_type === "newapi"
        ? await quotaApi.queryNewapi(baseUrl, p.api_key, p.extra ?? "", p.id)
        : await quotaApi.query(baseUrl, p.api_key, p.id);
      if (q.success) setQuotaMap(prev => ({ ...prev, [p.id]: q }));
    } catch { /* ignore */ }
    finally {
      setQuotaPending(prev => { const n = { ...prev }; delete n[p.id]; return n; });
    }
  }, []);

  // 有界并发池：共享队列 quotaQueueRef + 至多 QUOTA_CONCURRENCY 个 worker 循环领取。
  //   入队由 IntersectionObserver（可视/未折叠优先）+ 兜底全量补齐触发；scheduled 去重防重复拉。
  const pumpQuotaPool = useCallback(() => {
    const spawn = async () => {
      quotaPoolActiveRef.current++;
      try {
        for (;;) {
          const p = quotaQueueRef.current.shift();
          if (!p) break;
          await fetchQuotaForPlatform(p);
        }
      } finally {
        quotaPoolActiveRef.current--;
      }
    };
    while (quotaPoolActiveRef.current < QUOTA_CONCURRENCY && quotaQueueRef.current.length > 0) {
      void spawn();
    }
  }, [fetchQuotaForPlatform]);

  const enqueueQuota = useCallback((p: Platform) => {
    if (quotaScheduledRef.current.has(p.id)) return;
    if (!quotaWantMapRef.current.has(p.id)) return; // 非本轮需查平台（已结算/不需查）忽略
    quotaScheduledRef.current.add(p.id);
    quotaQueueRef.current.push(p);
    pumpQuotaPool();
  }, [pumpQuotaPool]);

  const scheduleQuotaFor = useCallback((p: Platform) => {
    if (!platformWantsQuota(p)) return;
    quotaWantMapRef.current.set(p.id, p);
    setQuotaPending(prev => ({ ...prev, [p.id]: true }));
    // 已调度过则先放行重查（编辑可能改了 key/base_url）。
    quotaScheduledRef.current.delete(p.id);
    enqueueQuota(p);
  }, [platformWantsQuota, enqueueQuota]);

  const refreshQuota = useCallback(async (p: Platform, opts?: { onNewapiUserId?: (uid: string) => void; onError?: (msg: string) => void }) => {
    if (!p.api_key) {
      opts?.onError?.(`${p.name}: ${t("platform.quotaNoKey", "缺少 API Key")}`);
      return;
    }
    // 手动刷新接管该平台 quota：清初始 pending（避免与 refreshing 旋转图标骨架重叠），显式调度去重也标记。
    setQuotaPending(prev => { const n = { ...prev }; delete n[p.id]; return n; });
    quotaScheduledRef.current.add(p.id);
    setQuotaRefreshing((s) => ({ ...s, [p.id]: true }));
    try {
      const baseUrl = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []) || p.base_url;
      const q = p.platform_type === "newapi"
        ? await quotaApi.queryNewapi(baseUrl, p.api_key, p.extra ?? "", p.id)
        : await quotaApi.query(baseUrl, p.api_key, p.id);
      if (q.success) {
        setQuotaMap((s) => ({ ...s, [p.id]: q }));
        setQuotaRealIds((s) => ({ ...s, [p.id]: true }));
        // New API: 自动回填 user_id
        if (p.platform_type === "newapi" && q.newapi_user_id) {
          opts?.onNewapiUserId?.(q.newapi_user_id);
        }
      } else {
        opts?.onError?.(`${p.name}: ${q.error || t("platform.quotaRefreshFail", "刷新额度失败")}`);
      }
    } catch (e) {
      console.error(e);
      opts?.onError?.(`${p.name}: ${t("platform.quotaRefreshFail", "刷新额度失败")}`);
    }
    setQuotaRefreshing((s) => ({ ...s, [p.id]: false }));
  }, [t]);

  const resetForLoad = useCallback((list: Platform[]) => {
    quotaQueueRef.current = [];
    quotaScheduledRef.current = new Set();
    const wantMap = new Map<number, Platform>();
    const pending: Record<number, boolean> = {};
    for (const p of list) {
      if (platformWantsQuota(p)) { wantMap.set(p.id, p); pending[p.id] = true; }
    }
    quotaWantMapRef.current = wantMap;
    setQuotaPending(pending);
    return { wantMap, pending };
  }, [platformWantsQuota]);

  return {
    quotaMap, quotaRealIds, quotaRefreshing, quotaPending,
    quotaQueueRef, quotaScheduledRef, quotaPoolActiveRef, quotaWantMapRef,
    platformWantsQuota, fetchQuotaForPlatform, enqueueQuota, scheduleQuotaFor, refreshQuota, resetForLoad,
    setQuotaMap, setQuotaPending, setQuotaRefreshing, setQuotaRealIds,
  };
}

export { getPrimaryBaseUrl };
