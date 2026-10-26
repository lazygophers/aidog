// GroupSidePanels — G3 分组页双侧边（整体风格重设计票 03，brief 21/22）。
// 左：静态候选序（前端按 sort_by_routing_mode 同构排序 + 状态过滤）与「下一个请求」预期路径；
// 右：最近一次路由决策快照（group_latest_routing，proxy_log.routing 最新一条）。同屏常驻，无点击切换。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { groupFlowApi, type Group, type GroupPlatformDetail, type LatestRouting } from "../../services/api";
import { routingModeLabel } from "../../domains/groups";
import { formatDateTime } from "../../utils/formatters";

/** 静态候选序：与后端 sort_by_routing_mode 同构的简化排序——
 *  仅启用的平台参与；failover/health_aware/sticky 按 (level_priority 降, priority 升) 定序；
 *  load_balance 同序但语义为权重分布（level_priority 视作权重近似）。 */
export function staticOrder(gps: GroupPlatformDetail[]): GroupPlatformDetail[] {
  return gps
    .filter(g => g.platform.status === "enabled")
    .sort((a, b) => (b.level_priority - a.level_priority) || (a.priority - b.priority));
}

export function GroupSidePanels({ group, gps }: { group: Group; gps: GroupPlatformDetail[] }) {
  const { t } = useTranslation();
  const [latest, setLatest] = useState<LatestRouting | null>(null);

  useEffect(() => {
    let cancelled = false;
    groupFlowApi.latestRouting(group.group_key)
      .then(r => { if (!cancelled) setLatest(r); })
      .catch(() => { if (!cancelled) setLatest(null); });
    return () => { cancelled = true; };
  }, [group.group_key]);

  const ordered = staticOrder(gps);
  const isLB = group.routing_mode === "load_balance";
  const weightSum = ordered.reduce((s, g) => s + Math.max(1, g.level_priority), 0);

  return (
    <div id="g3-side-panels" style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
      {/* ── 左：候选序 + 下一个请求 ── */}
      <div id="g3-candidates" className="glass-surface" style={{ padding: "12px 14px", display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span style={{ fontSize: 12.5, fontWeight: 600 }}>{t("group.candidates.title", "候选序")}</span>
          <span className="text-tertiary" style={{ fontSize: 11.5, fontFamily: "var(--font-mono, monospace)" }}>{routingModeLabel(t, group.routing_mode)}</span>
        </div>
        {ordered.length === 0 ? (
          <div className="text-secondary" style={{ fontSize: 12 }}>{t("group.candidates.none", "无启用中的候选平台")}</div>
        ) : (
          <>
            <div id="g3-next-request" style={{
              fontSize: 12, padding: "6px 9px", borderRadius: "var(--radius-sm)",
              background: "var(--accent-subtle)", color: "var(--primary)",
              display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap",
            }}>
              <span style={{ fontWeight: 600 }}>{t("group.next.title", "下一个请求")}</span>
              <span>→</span>
              {isLB
                ? t("group.next.distributed", { defaultValue: "按权重分布（{{n}} 个候选）", n: ordered.length })
                : <b style={{ fontFamily: "var(--font-mono, monospace)" }}>{ordered[0].platform.platform_type}</b>}
            </div>
            {ordered.map((g, i) => (
              <div key={g.platform.id} id={`g3-cand-${g.platform.id}`} style={{
                display: "flex", gap: 8, alignItems: "center", fontSize: 11.5,
                padding: "3px 6px", borderRadius: "var(--radius-sm)",
                background: i === 0 && !isLB ? "var(--accent-subtle)" : "transparent",
              }}>
                <span className="text-tertiary" style={{ fontFamily: "var(--font-mono, monospace)", minWidth: 14 }}>{i + 1}</span>
                <span style={{ minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{g.platform.platform_type}</span>
                {g.platform.status !== "enabled" && (
                  <span style={{ fontSize: 11, color: "var(--color-warning)" }}>{t("group.candidates.disabled", "停用")}</span>
                )}
                <span style={{ flex: 1 }} />
                <span className="text-tertiary" title={t("group.candidates.posReason", "优先级 {{level}} / 序 {{prio}}", { level: g.level_priority, prio: g.priority })}>
                  {isLB ? `${Math.round(Math.max(1, g.level_priority) / weightSum * 100)}%` : `L${g.level_priority}`}
                </span>
              </div>
            ))}
          </>
        )}
      </div>

      {/* ── 右：最近路由快照 ── */}
      <div id="g3-latest-routing" className="glass-surface" style={{ padding: "12px 14px", display: "flex", flexDirection: "column", gap: 6 }}>
        <span style={{ fontSize: 12.5, fontWeight: 600 }}>{t("group.routing.latest", "最近路由")}</span>
        {!latest ? (
          <div className="text-secondary" style={{ fontSize: 12 }}>{t("group.routing.none", "暂无路由记录")}</div>
        ) : (
          <>
            <div style={{ fontSize: 11.5, display: "flex", gap: 6, flexWrap: "wrap" }}>
              <b id="g3-latest-selected" style={{ fontFamily: "var(--font-mono, monospace)", color: "var(--primary)" }}>
                {latest.snapshot.entries.find(e => e.reason === "selected")?.platform ?? `#${latest.platform_id}`}
              </b>
              <span className="text-tertiary" style={{ fontFamily: "var(--font-mono, monospace)" }}>{latest.source_model}</span>
              <span className="text-tertiary">{formatDateTime(latest.created_at)}</span>
            </div>
            {latest.snapshot.entries.slice(0, 5).map((e, i) => (
              <div key={i} style={{ display: "flex", gap: 8, fontSize: 11, alignItems: "center" }}>
                <span data-reason={e.reason} style={{
                  minWidth: 40, fontSize: 11, padding: "0 5px", borderRadius: 4,
                  background: e.reason === "selected" ? "var(--accent-subtle)" : "var(--secondary)",
                  color: e.reason === "selected" ? "var(--primary)" : "var(--muted-foreground)",
                }}>{e.reason}</span>
                <span style={{ minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{e.platform}</span>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}
