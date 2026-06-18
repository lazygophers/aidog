import { useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { Platform } from "../../services/api";
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
  setTestingPlatform: React.Dispatch<React.SetStateAction<Platform | null>>;
  // Actions
  refreshQuota: (p: Platform) => Promise<void>;
  toggleExpanded: (id: number, next: boolean) => void;
  handleQuickTest: (p: Platform) => Promise<void>;
  handleToggle: (p: Platform) => Promise<void>;
  handleViewLogs: (p: Platform) => void;
  handleDelete: (id: number) => Promise<void>;
  handleEdit: (p: Platform) => void;
  handleCustomTest: (p: Platform) => void;
  onFaviconFailed: (fn: (prev: Set<number>) => Set<number>) => void;
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
  void testingPlatform; // Suppress unused warning（由父组件通过 setTestingPlatform 使用）

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
    try {
      const defaultModel = p.models.default || p.available_models[0] || "";
      const r = await modelTestApi.test({ platform_id: p.id, model: defaultModel, max_tokens: 64 });
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
  }, [t, setToast]);

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
    setTestingPlatform,
    refreshQuota,
    toggleExpanded,
    handleQuickTest,
    handleToggle,
    handleViewLogs,
    handleDelete,
    handleEdit,
    handleCustomTest,
    onFaviconFailed,
  };
}

// ── 导出辅助函数供外部使用 ──
export { computeQuotaDisplay, computeManualBudgetDisplay };
