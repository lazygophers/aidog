import { useState, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { Platform, LastTestResult, SharePlatform } from "../../services/api";
import { platformApi, quotaApi, modelTestApi } from "../../services/api";
import {
  computeQuotaDisplay,
  computeManualBudgetDisplay,
} from "../../pages/Platforms";

// ── Types ──

interface UsePlatformCardsOptions {
  /** 导航回调（跳转编辑/日志） */
  onNavigate?: (id: string, context?: { platformId?: number; platformName?: string }) => void;
  /** 编辑平台回调（打开表单） */
  onEdit?: (p: Platform) => void;
  /** Toast 提示回调 */
  setToast?: (toast: { text: string; ok: boolean } | null) => void;
}

interface UsePlatformCardsReturn {
  // States
  usageMap: Record<number, import("../../services/api").PlatformUsageStats>;
  quotaMap: Record<number, import("../../services/api").PlatformQuota>;
  quotaRealIds: Record<number, boolean>;
  quotaRefreshing: Record<number, boolean>;
  testResults: Record<number, "ok" | "fail">;
  faviconFailed: Set<number>;
  expandedIds: Set<number>;
  testingId: number | null;
  setUsageMap: React.Dispatch<React.SetStateAction<Record<number, import("../../services/api").PlatformUsageStats>>>;
  setQuotaMap: React.Dispatch<React.SetStateAction<Record<number, import("../../services/api").PlatformQuota>>>;
  setTestResults: React.Dispatch<React.SetStateAction<Record<number, "ok" | "fail">>>;
  setTestingId: React.Dispatch<React.SetStateAction<number | null>>;
  testingPlatform: Platform | null;
  setTestingPlatform: React.Dispatch<React.SetStateAction<Platform | null>>;
  lastTestMap: Record<number, LastTestResult>;
  setLastTestMap: React.Dispatch<React.SetStateAction<Record<number, LastTestResult>>>;
  refreshLastTest: (platformId: number) => Promise<void>;
  // Actions
  refreshQuota: (p: Platform) => Promise<void>;
  toggleExpanded: (id: number, next: boolean) => void;
  handleQuickTest: (p: Platform) => Promise<void>;
  handleToggle: (p: Platform) => Promise<void>;
  handleViewLogs: (p: Platform) => void;
  handleDelete: (id: number) => Promise<void>;
  handleEdit: (p: Platform) => void;
  handleShare: (p: Platform) => Promise<void>;
  handleCustomTest: (p: Platform) => void;
  onFaviconFailed: (fn: (prev: Set<number>) => Set<number>) => void;
  // Share modal state
  shareData: { share: SharePlatform; name: string } | null;
  setShareData: React.Dispatch<React.SetStateAction<{ share: SharePlatform; name: string } | null>>;
}

// Helper: 从 endpoints 中推导主 base_url
function getPrimaryBaseUrl(proto: Platform["platform_type"], eps: Platform["endpoints"]): string {
  const endpoints = eps ?? [];
  const primary = endpoints.find(ep => ep.protocol === proto);
  if (primary) return primary.base_url;
  return endpoints[0]?.base_url || "";
}

// ── Hook ──

export function usePlatformCards(options?: UsePlatformCardsOptions): UsePlatformCardsReturn {
  const { t } = useTranslation();
  const { onNavigate, onEdit: onEditProp, setToast: setToastProp } = options ?? {};

  const [usageMap, setUsageMap] = useState<Record<number, import("../../services/api").PlatformUsageStats>>({});
  const [quotaMap, setQuotaMap] = useState<Record<number, import("../../services/api").PlatformQuota>>({});
  const [quotaRealIds, setQuotaRealIds] = useState<Record<number, boolean>>({});
  const [quotaRefreshing, setQuotaRefreshing] = useState<Record<number, boolean>>({});
  const [testResults, setTestResults] = useState<Record<number, "ok" | "fail">>({});
  const [faviconFailed, setFaviconFailed] = useState<Set<number>>(new Set());
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());
  const [testingId, setTestingId] = useState<number | null>(null);
  const [testingPlatform, setTestingPlatform] = useState<Platform | null>(null);
  const [lastTestMap, setLastTestMap] = useState<Record<number, LastTestResult>>({});
  const [shareData, setShareData] = useState<{ share: SharePlatform; name: string } | null>(null);

  // 默认的 toast 设置函数
  const setToast = setToastProp ?? (() => {});

  // Toggle expanded
  const toggleExpanded = useCallback((id: number, next: boolean) => {
    setExpandedIds(prev => {
      const s = new Set(prev);
      if (next) s.add(id); else s.delete(id);
      return s;
    });
  }, []);

  // Refresh quota
  const refreshQuota = useCallback(async (p: Platform) => {
    if (!p.api_key) {
      setToast({ text: `${p.name}: ${t("platform.quotaNoKey", "缺少 API Key")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
      return;
    }
    setQuotaRefreshing((s) => ({ ...s, [p.id]: true }));
    try {
      const baseUrl = getPrimaryBaseUrl(p.platform_type, p.endpoints ?? []) || p.base_url;
      const q = p.platform_type === "newapi"
        ? await quotaApi.queryNewapi(baseUrl, p.api_key, p.extra ?? "", p.id)
        : await quotaApi.query(baseUrl, p.api_key, p.id);
      if (q.success) {
        setQuotaMap((s) => ({ ...s, [p.id]: q }));
        setQuotaRealIds((s) => ({ ...s, [p.id]: true }));
      } else {
        setToast({ text: `${p.name}: ${q.error || t("platform.quotaRefreshFail", "刷新额度失败")}`, ok: false });
        setTimeout(() => setToast(null), 3000);
      }
    } catch (e) {
      console.error(e);
      setToast({ text: `${p.name}: ${t("platform.quotaRefreshFail", "刷新额度失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
    setQuotaRefreshing((s) => ({ ...s, [p.id]: false }));
  }, [t, setToast]);

  // Quick test
  const handleQuickTest = useCallback(async (p: Platform) => {
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
    // 派发全局事件：跨页（Platforms/Groups/ModelTestPanel）订阅者据此单卡刷新「最近测试」徽章 + testResults（health）
    window.dispatchEvent(new CustomEvent("aidog-platform-test-completed", { detail: { platformId: p.id, success } }));
  }, [t, setToast]);

  // 取某平台最近一次测试日志（proxy_log source_protocol='test'），刷新 lastTestMap 对应项
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

  // 监听全局测试完成事件：单卡刷新徽章数据 + 写 testResults（驱动 health 走 manual 分支，
  // 来自其它页/批量测试的成功/失败信号也即时反映到本页健康点，不必等本卡单测）
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

  // Toggle enabled
  const handleToggle = useCallback(async (p: Platform) => {
    try {
      const nextStatus = p.status === "enabled" ? "disabled" : "enabled";
      await platformApi.update({ id: p.id, status: nextStatus });
      // 触发重新加载（由父组件处理）
    } catch (e) { console.error(e); }
  }, []);

  // View logs
  const handleViewLogs = useCallback((p: Platform) => {
    onNavigate?.("logs", { platformId: p.id, platformName: p.name });
  }, [onNavigate]);

  // Delete
  const handleDelete = useCallback(async (id: number) => {
    try { await platformApi.delete(id); } catch (e) { console.error(e); }
  }, []);

  // Edit
  const handleEdit = useCallback((p: Platform) => {
    if (onEditProp) {
      onEditProp(p);
    } else {
      // 默认行为：跳转到编辑页
      onNavigate?.("platforms", { platformId: p.id, platformName: p.name });
    }
  }, [onEditProp, onNavigate]);

  // Share（导出可分享配置 → 打开 ShareModal）
  const handleShare = useCallback(async (p: Platform) => {
    try {
      const share = await platformApi.shareExport(p.id);
      setShareData({ share, name: p.name });
    } catch (e) {
      console.error(e);
      setToast({ text: `${p.name}: ${t("platform.share.exportFail", "导出分享内容失败")}`, ok: false });
      setTimeout(() => setToast(null), 3000);
    }
  }, [t, setToast]);

  // Custom test（由父组件处理）
  const handleCustomTest = useCallback((p: Platform) => {
    setTestingPlatform(p);
  }, []);

  // Favicon failed
  const onFaviconFailed = useCallback((fn: (prev: Set<number>) => Set<number>) => {
    setFaviconFailed(fn);
  }, []);

  return {
    usageMap,
    quotaMap,
    quotaRealIds,
    quotaRefreshing,
    testResults,
    faviconFailed,
    expandedIds,
    testingId,
    setUsageMap,
    setQuotaMap,
    setTestResults,
    setTestingId,
    testingPlatform,
    setTestingPlatform,
    lastTestMap,
    setLastTestMap,
    refreshLastTest,
    refreshQuota,
    toggleExpanded,
    handleQuickTest,
    handleToggle,
    handleViewLogs,
    handleDelete,
    handleEdit,
    handleShare,
    handleCustomTest,
    onFaviconFailed,
    shareData,
    setShareData,
  };
}

// ── 导出辅助函数供外部使用 ──
export { computeQuotaDisplay, computeManualBudgetDisplay };
