import { useLogsData } from "./Logs/useLogsData";
import { DetailPanel } from "./Logs/DetailPanel";
import { ListView } from "./Logs/ListView";

export function Logs({ initialFilter }: { initialFilter?: { platformId?: number; platformName?: string; groupId?: string; groupKey?: string } }) {
  const d = useLogsData(initialFilter);
  if (d.detail) return <DetailPanel d={d} />;
  return <ListView d={d} />;
}
