// HomeFlowCard — 首页「请求流量」卡（请求流向票 05）：客户端 → 分组 → 请求模型 → 平台 → 实际模型，恒 24h。
// 数据由 Home.load 拉取传入；度量切换 / 占比 / Top N 在 FlowChart 前端算。外壳走 PANEL 显式色（同 HomeTrendChart）。
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { FlowRow, Platform } from "../services/api";
import { FlowChart, type FlowMetric } from "../components/charts/FlowChart";
import { useDisplayCurrency } from "../utils/displayCurrency";
import { clientLabel } from "../utils/clientLabel";
import { usePanel } from "./Home";

const METRICS: FlowMetric[] = ["tokens", "requests", "cost"];

export function HomeFlowCard({ rows, platforms, ungroupedLabel, loading }: {
  rows: FlowRow[];
  platforms: Platform[];
  ungroupedLabel: string;
  loading: boolean;
}) {
  useDisplayCurrency();
  const P = usePanel();
  const { t } = useTranslation();
  const [metric, setMetric] = useState<FlowMetric>("tokens");

  // 列 key 同时是 FlowChart 的 testid 后缀（flow-col-<key>），故把 FlowRow 字段改名成规格里的列名。
  const chartRows = useMemo(() => rows.map(r => ({
    client: r.client, group: r.group_key, requestModel: r.req_model, platform: r.platform_id, actualModel: r.model,
    tokens: r.tokens, requests: r.requests, cost: r.est_cost,
  })), [rows]);

  const columns = useMemo(() => {
    const platName = (id: string) => (id === "0" ? t("flow.platform.none", "未路由") : (platforms.find(p => String(p.id) === id)?.platform_type ?? `#${id}`));
    return [
      { key: "client", title: t("flow.col.client", "客户端"), labelOf: (id: string) => clientLabel(t, id) },
      { key: "group", title: t("flow.col.group", "分组"), labelOf: (id: string) => (id === "" ? ungroupedLabel : id) },
      { key: "requestModel", title: t("flow.col.requestModel", "请求模型") },
      { key: "platform", title: t("flow.col.platform", "平台"), labelOf: platName },
      { key: "actualModel", title: t("flow.col.actualModel", "实际模型") },
    ];
  }, [t, platforms, ungroupedLabel]);

  const btn = (on: boolean) => ({ all: "unset" as const, cursor: "pointer", fontSize: 11, padding: "2px 9px", borderRadius: "var(--radius-sm)",
    background: on ? "var(--accent-subtle)" : "transparent", color: on ? "var(--primary)" : P.muted });

  return (
    <div data-testid="home-flow" data-loading={loading ? "yes" : "no"} style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
        <span style={{ fontSize: 12.5, fontWeight: 600, color: P.fg }}>{t("home.flow.title", "请求流量 · 24 小时")}</span>
        <div style={{ display: "flex", gap: 4 }}>
          {METRICS.map(m => (
            <button key={m} type="button" data-testid={`flow-metric-${m}`} data-state={metric === m ? "on" : "off"} onClick={() => setMetric(m)} style={btn(metric === m)}>
              {t(`group.flow.${m}`, m)}
            </button>
          ))}
        </div>
      </div>
      {!loading && rows.length === 0 ? (
        <div data-testid="flow-empty" style={{ fontSize: 12, padding: "18px 0", textAlign: "center", color: P.muted }}>
          {t("group.flow.empty", "窗口内无请求")}
        </div>
      ) : (
        <FlowChart columns={columns} rows={chartRows} metric={metric} surface={P.s1} />
      )}
    </div>
  );
}
