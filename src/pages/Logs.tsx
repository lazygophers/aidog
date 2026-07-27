import { useLogsFilters } from "./Logs/useLogsFilters";
import { useLogsList } from "./Logs/useLogsList";
import { useLogsDetail } from "./Logs/useLogsDetail";
import { DetailPanel } from "./Logs/DetailPanel";
import { ListView } from "./Logs/ListView";

export function Logs({ initialFilter }: { initialFilter?: { platformId?: number; platformName?: string; groupId?: string; groupKey?: string } }) {
  const filters = useLogsFilters(initialFilter);
  const list = useLogsList(filters.activeFilter, filters.hasFilter);
  const detail = useLogsDetail();
  // DetailPanel 现为 Sheet 叠加层（Radix Portal），恒常渲染；open = detail 非空。
  // 列表始终可见，详情以右侧抽屉展开（不再整页替换）。
  return (
    <>
      <ListView filters={filters} list={list} openDetail={detail.openDetail} copyRow={detail.copyRow} />
      <DetailPanel d={{ ...detail, platformMap: filters.platformMap, groupName: filters.groupName }} />
    </>
  );
}
