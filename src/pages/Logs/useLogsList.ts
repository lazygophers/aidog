import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { proxyLogApi, onProxyLogUpdated, type ProxyLogSummary, type ProxyLogFilter } from "../../services/api";

const DEFAULT_PAGE_SIZE = 20;

/**
 * Logs 页列表态：分页/加载/轮询刷新 + 清空/清理过期。
 * 依赖调用方（filters 段）提供的 activeFilter/hasFilter，自 useLogsData 拆出，行为不变。
 */
export function useLogsList(activeFilter: ProxyLogFilter, hasFilter: boolean) {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<ProxyLogSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [pageSize, setPageSize] = useState<number>(DEFAULT_PAGE_SIZE);
  const [loading, setLoading] = useState(true);
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [cleanupMessage, setCleanupMessage] = useState<string>("");

  const load = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    try {
      // 始终走 filtered 路径：activeFilter 默认带 exclude_sources=["test","quota"]
      const [items, count] = await Promise.all([
        proxyLogApi.listFiltered(activeFilter, pageSize, offset),
        proxyLogApi.countFiltered(activeFilter),
      ]);
      setLogs(items || []);
      setTotal(count);
    } catch (e) { console.error(e); }
    if (!silent) setLoading(false);
  }, [offset, pageSize, activeFilter]);

  useEffect(() => { load(); }, [load]);
  useEffect(() => { setOffset(0); }, [hasFilter, activeFilter, pageSize]);

  const refreshList = useCallback(() => { load(true); }, [load]);
  useEffect(() => onProxyLogUpdated(() => { refreshList(); }, 500), [refreshList]);

  const handleClear = async () => {
    try {
      await proxyLogApi.clear();
      setShowClearConfirm(false);
      setOffset(0);
      load();
    } catch (e) { console.error(e); }
  };

  const handleCleanupExpired = async () => {
    try {
      await proxyLogApi.cleanupExpired();
      setOffset(0);
      load();
      setCleanupMessage(t("logs.cleanupExpiredDone", "已清理过期日志"));
      setTimeout(() => setCleanupMessage(""), 3000);
    } catch (e) { console.error(e); }
  };

  return {
    logs, total, offset, pageSize, loading, setOffset, setPageSize, load,
    handleClear, handleCleanupExpired, showClearConfirm, setShowClearConfirm, cleanupMessage,
  };
}

export type LogsListData = ReturnType<typeof useLogsList>;
