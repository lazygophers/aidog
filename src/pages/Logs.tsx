import { useLogsData } from "./Logs/useLogsData";
import { DetailPanel } from "./Logs/DetailPanel";
import { ListView } from "./Logs/ListView";

export function Logs({ initialFilter }: { initialFilter?: { platformId?: number; platformName?: string; groupId?: string; groupKey?: string } }) {
  const d = useLogsData(initialFilter);
  // DetailPanel 现为 Sheet 叠加层（Radix Portal），恒常渲染；open = detail 非空。
  // 列表始终可见，详情以右侧抽屉展开（不再整页替换）。
  return (
    <>
      <ListView d={d} />
      <DetailPanel d={d} />
    </>
  );
}
