// GroupFlowPanel — G3 分组页桑基 flow（整体风格重设计票 03，brief 20/23）。
// 模型 → 平台 两列带向图：线宽 = 度量（请求/费用），7d/today × 请求/费用 段切换，
// hover 显示 per-link 明细。数据源 groupFlowApi.breakdown（stats_agg_hourly GROUP BY）。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { groupFlowApi, type GroupPlatformDetail } from "../../services/api";
import { formatNumber, formatCost } from "../../utils/formatters";

type Window = "7d" | "today";
type Metric = "requests" | "cost";

interface Link { model: string; platformId: number; value: number }

export function GroupFlowPanel({ groupKey, gps }: { groupKey: string; gps: GroupPlatformDetail[] }) {
  const { t } = useTranslation();
  const [win, setWin] = useState<Window>("7d");
  const [metric, setMetric] = useState<Metric>("requests");
  const [links, setLinks] = useState<Link[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    groupFlowApi.breakdown(groupKey, win, true)
      .then(rows => {
        if (cancelled) return;
        setLinks(rows
          .filter(r => r.model)
          .map(r => ({ model: r.model!, platformId: r.platform_id, value: metric === "requests" ? r.requests : r.est_cost })));
      })
      .catch(() => { if (!cancelled) setLinks([]); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [groupKey, win, metric]);

  const platName = (id: number) => gps.find(g => g.platform.id === id)?.platform.platform_type ?? `#${id}`;
  // 两列节点：模型（左，Top 6 + 其余归并） / 平台（右，按总量降序）
  const modelTotals = new Map<string, number>();
  const platTotals = new Map<number, number>();
  for (const l of links) {
    modelTotals.set(l.model, (modelTotals.get(l.model) ?? 0) + l.value);
    platTotals.set(l.platformId, (platTotals.get(l.platformId) ?? 0) + l.value);
  }
  const topModels = [...modelTotals.entries()].sort((a, b) => b[1] - a[1]).slice(0, 6).map(e => e[0]);
  const models = topModels.length < modelTotals.size ? [...topModels, "__other__"] : topModels;
  const plats = [...platTotals.entries()].sort((a, b) => b[1] - a[1]).map(e => e[0]);
  const total = links.reduce((s, l) => s + l.value, 0);
  const merged = links.map(l => ({ ...l, model: topModels.includes(l.model) ? l.model : "__other__" }));

  const H = Math.max(models.length, plats.length) * 26 + 16;
  const colL = 130, colR = 250;
  const W = colR + 120;
  const scale = (v: number) => (total > 0 ? v / total : 0);
  // 左列节点纵向均分；连线 y 取两端节点中心，粗细按值占比（上限 14px）
  const rowY = (i: number, n: number) => (n <= 1 ? H / 2 : 14 + (i * (H - 28)) / (n - 1));
  const fmtVal = (v: number) => metric === "requests" ? formatNumber(Math.round(v)) : `$${formatCost(v)}`;

  return (
    <div id="g3-flow" data-loading={loading ? "yes" : "no"} className="glass-surface"
      style={{ padding: "12px 14px", display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
        <span style={{ fontSize: 12.5, fontWeight: 600 }}>{t("group.flow.title", "请求流向")}</span>
        <div style={{ display: "flex", gap: 4 }}>
          {(["7d", "today"] as Window[]).map(w => (
            <button key={w} type="button" data-state={win === w ? "on" : "off"} onClick={() => setWin(w)}
              style={{ all: "unset", cursor: "pointer", fontSize: 11, padding: "2px 9px", borderRadius: "var(--radius-sm)",
                background: win === w ? "var(--accent-subtle)" : "transparent", color: win === w ? "var(--primary)" : "var(--text-secondary)" }}>
              {t(`group.flow.${w}`, w === "7d" ? "近 7 天" : "今天")}
            </button>
          ))}
          <span style={{ width: 8 }} />
          {(["requests", "cost"] as Metric[]).map(m => (
            <button key={m} type="button" data-state={metric === m ? "on" : "off"} onClick={() => setMetric(m)}
              style={{ all: "unset", cursor: "pointer", fontSize: 11, padding: "2px 9px", borderRadius: "var(--radius-sm)",
                background: metric === m ? "var(--accent-subtle)" : "transparent", color: metric === m ? "var(--primary)" : "var(--text-secondary)" }}>
              {t(`group.flow.${m}`, m === "requests" ? "请求量" : "费用")}
            </button>
          ))}
        </div>
      </div>
      {!loading && links.length === 0 ? (
        <div className="text-secondary" style={{ fontSize: 12, padding: "18px 0", textAlign: "center" }}>
          {t("group.flow.empty", "窗口内无请求")}
        </div>
      ) : (
        <svg id="g3-flow-svg" width="100%" viewBox={`0 0 ${W} ${H}`} style={{ maxHeight: 240, fontFamily: "var(--font-mono, monospace)" }} role="img">
          {merged.map((l, i) => {
            const mi = models.indexOf(l.model);
            const pi = plats.indexOf(l.platformId);
            const y1 = rowY(mi, models.length), y2 = rowY(pi, plats.length);
            const w = Math.max(1, scale(l.value) * 14);
            return (
              <path key={i}
                d={`M ${colL} ${y1} C ${(colL + colR) / 2} ${y1}, ${(colL + colR) / 2} ${y2}, ${colR} ${y2}`}
                stroke={`var(--chart-${(pi % 4) + 1})`} strokeWidth={w} fill="none" opacity={0.45}
                strokeLinecap="round">
                <title>{`${l.model === "__other__" ? t("group.flow.other", "其他") : l.model} → ${platName(l.platformId)}：${fmtVal(l.value)}`}</title>
              </path>
            );
          })}
          {models.map((m, i) => {
            const y = rowY(i, models.length);
            const v = m === "__other__"
              ? merged.filter(l => l.model === "__other__").reduce((s, l) => s + l.value, 0)
              : modelTotals.get(m) ?? 0;
            return (
              <text key={m} x={colL - 8} y={y + 3.5} textAnchor="end" fontSize={10.5} fill="var(--muted-foreground)">
                {m === "__other__" ? t("group.flow.other", "其他") : m}
                <tspan fill="var(--text-tertiary)" dx={6}>{fmtVal(v)}</tspan>
              </text>
            );
          })}
          {plats.map((p, i) => {
            const y = rowY(i, plats.length);
            return (
              <text key={p} x={colR + 8} y={y + 3.5} fontSize={10.5} fill="var(--foreground)">
                {platName(p)}
                <tspan fill="var(--text-tertiary)" dx={6}>{fmtVal(platTotals.get(p) ?? 0)}</tspan>
              </text>
            );
          })}
        </svg>
      )}
    </div>
  );
}
