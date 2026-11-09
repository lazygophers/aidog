import { useEffect } from "react";
import { useLogsFilters } from "./Logs/useLogsFilters";
import { useLogsList } from "./Logs/useLogsList";
import { useLogsDetail } from "./Logs/useLogsDetail";
import { DetailPanel } from "./Logs/DetailPanel";
import { ListView } from "./Logs/ListView";
import { useDetailScroll } from "./Logs/useDetailScroll";

export function Logs({ initialFilter, onNavigate }: { onNavigate?: (id: string) => void; initialFilter?: { platformId?: number; platformName?: string; groupId?: string; groupKey?: string; logId?: string; sessionKey?: string } }) {
  const filters = useLogsFilters(initialFilter);
  const list = useLogsList(filters.activeFilter, filters.hasFilter);
  const detail = useLogsDetail();
  const logId = initialFilter?.logId;
  // eslint-disable-next-line react-hooks/exhaustive-deps -- 仅在 logId 变化时打开一次
  useEffect(() => { if (logId) detail.openDetail(logId); }, [logId]);
  useDetailScroll(detail.detail !== null);
  // 详情是独立全页：detail 非空时替换列表；筛选 / 分页状态在上面的 hook 里，返回后保持。
  if (detail.detail !== null) {
    return <DetailPanel d={{ ...detail, platformMap: filters.platformMap, groupName: filters.groupName, onNavigate }} />;
  }
  return <ListView filters={filters} list={list} openDetail={detail.openDetail} copyRow={detail.copyRow} />;
}
