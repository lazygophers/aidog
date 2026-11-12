// GroupFlowPanel — 分组页请求流向（请求流向票 04）：客户端 → 请求类型 → 请求模型 → 平台 → 实际模型。
// 数据源 groupFlowApi.flow（stats_flow_breakdown）；度量切换 / 占比 / Top N 全在 FlowChart 前端算。
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { groupFlowApi, type FlowRow, type GroupPlatformDetail } from "../../services/api";
import { FlowChart, type FlowMetric } from "../../components/charts/FlowChart";
import { Flow3DChart } from "../../components/charts/Flow3DChart";
import { useDisplayCurrency } from "../../utils/displayCurrency";
import { clientLabel } from "../../utils/clientLabel";
import { requestClassLabel } from "../../utils/requestClass";

type Window = "7d" | "today";
const METRICS: FlowMetric[] = ["tokens", "requests", "cost"];

export function GroupFlowPanel({ groupKey, gps }: { groupKey: string; gps: GroupPlatformDetail[] }) {
  useDisplayCurrency();
  const { t } = useTranslation();
  const [win, setWin] = useState<Window>("today");
  const [metric, setMetric] = useState<FlowMetric>("tokens");
  // 视图选择只放组件 state，不持久化：重挂载回到桑基
  const [view, setView] = useState<"sankey" | "3d">("sankey");
  const [rows, setRows] = useState<FlowRow[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    groupFlowApi.flow(win, groupKey)
      .then(r => { if (!cancelled) setRows(r); })
      .catch(() => { if (!cancelled) setRows([]); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [groupKey, win]);

  const columns = useMemo(() => {
    const platName = (id: string) => (id === "0" ? t("flow.platform.none", "未路由") : (gps.find(g => String(g.platform.id) === id)?.platform.name ?? `#${id}`));
    return [
      { key: "client", title: t("flow.col.client", "客户端"), labelOf: (id: string) => clientLabel(t, id) },
      { key: "request_class", title: t("flow.col.requestClass", "请求类型"), labelOf: (id: string) => (id === "" ? t("flow.requestClass.unknown", "未知") : requestClassLabel(t, id)) },
      { key: "req_model", title: t("flow.col.requestModel", "请求模型") },
      { key: "platform_id", title: t("flow.col.platform", "平台"), labelOf: platName },
      { key: "model", title: t("flow.col.actualModel", "实际模型") },
    ];
  }, [t, gps]);

  const btn = (on: boolean) => ({ all: "unset" as const, cursor: "pointer", fontSize: 14, padding: "2px 9px", borderRadius: "var(--radius-sm)",
    background: on ? "var(--accent-subtle)" : "transparent", color: on ? "var(--primary)" : "var(--text-secondary)" });

  return (
    <div id="g3-flow" data-loading={loading ? "yes" : "no"} className="glass-surface"
      style={{ padding: "12px 14px", display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
        <span style={{ fontSize: 14, fontWeight: 600 }}>{t("group.flow.title", "请求流向")}</span>
        <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
          <button type="button" data-testid="flow-view-toggle" data-state={view} onClick={() => setView(v => (v === "3d" ? "sankey" : "3d"))} style={btn(false)}>
            {view === "3d" ? t("flow.view.sankey", "桑基图") : t("flow.view.3d", "3D 视图")}
          </button>
          <span style={{ width: 8 }} />
          {(["7d", "today"] as Window[]).map(w => (
            <button key={w} type="button" data-testid={`flow-win-${w}`} data-state={win === w ? "on" : "off"} onClick={() => setWin(w)} style={btn(win === w)}>
              {t(`group.flow.${w}`, w === "7d" ? "近 7 天" : "今天")}
            </button>
          ))}
          <span style={{ width: 8 }} />
          {METRICS.map(m => (
            <button key={m} type="button" data-testid={`flow-metric-${m}`} data-state={metric === m ? "on" : "off"} onClick={() => setMetric(m)} style={btn(metric === m)}>
              {t(`group.flow.${m}`, m)}
            </button>
          ))}
        </div>
      </div>
      {!loading && rows.length === 0 ? (
        <div data-testid="flow-empty" className="text-secondary" style={{ fontSize: 14, padding: "18px 0", textAlign: "center" }}>
          {t("group.flow.empty", "窗口内无请求")}
        </div>
      ) : (
        <div id="g3-flow-svg">{view === "3d"
          ? <Flow3DChart columns={columns} rows={rows as unknown as Record<string, string | number>[]} metric={metric} />
          : <FlowChart columns={columns} rows={rows as unknown as Record<string, string | number>[]} metric={metric} />}</div>
      )}
    </div>
  );
}
