import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  statsApi,
  groupDetailApi,
  platformApi,
  type StatsResult,
  type StatsQuery,
  type GroupDetail,
  type Platform,
} from "../services/api";

const F = { title: 20, label: 15, body: 15, hint: 13, small: 12 } as const;

type TimePreset = "today" | "7d" | "30d" | "custom";

function formatNumber(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1) + "K";
  return n.toFixed(n % 1 === 0 ? 0 : 1);
}

function getTimeRange(preset: TimePreset, customStart?: number, customEnd?: number): { start: number; end: number } {
  const now = new Date();
  const ms = (d: Date) => d.getTime();
  switch (preset) {
    case "today": {
      const s = new Date(now); s.setHours(0, 0, 0, 0);
      return { start: ms(s), end: ms(now) };
    }
    case "7d": {
      const s = new Date(now); s.setDate(s.getDate() - 7);
      return { start: ms(s), end: ms(now) };
    }
    case "30d": {
      const s = new Date(now); s.setDate(s.getDate() - 30);
      return { start: ms(s), end: ms(now) };
    }
    case "custom":
      return { start: customStart ?? (now.getTime() - 7 * 86400000), end: customEnd ?? now.getTime() };
  }
}

export function Stats() {
  const { t } = useTranslation();
  const [data, setData] = useState<StatsResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [preset, setPreset] = useState<TimePreset>("today");
  const [granularity, setGranularity] = useState<"daily" | "hourly">("daily");
  const [groupBy, setGroupBy] = useState<"platform" | "model" | "group">("platform");
  const [filterGroup, setFilterGroup] = useState("");
  const [filterModel, setFilterModel] = useState("");
  const [filterProtocol, setFilterProtocol] = useState("");
  const [groups, setGroups] = useState<GroupDetail[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const range = getTimeRange(preset);
      const query: StatsQuery = {
        start: range.start,
        end: range.end,
        granularity,
        group_by: groupBy,
        filter_group: filterGroup || undefined,
        filter_model: filterModel || undefined,
        filter_protocol: filterProtocol || undefined,
      };
      const result = await statsApi.query(query);
      setData(result);
    } catch (e) {
      console.error(e);
    }
    setLoading(false);
  }, [preset, granularity, groupBy, filterGroup, filterModel, filterProtocol]);

  useEffect(() => { load(); }, [load]);

  // Load filter options
  useEffect(() => {
    groupDetailApi.list().then(setGroups).catch(() => {});
    platformApi.list().then(setPlatforms).catch(() => {});
  }, []);

  // Collect unique models from groups
  const allModels = Array.from(new Set(groups.flatMap(g => g.model_mappings.map(m => m.target_model)))).sort();

  const overview = data?.overview;
  const buckets = data?.buckets ?? [];
  const dims = data?.dimension_data ?? [];

  // Chart scaling
  const maxReq = Math.max(1, ...buckets.map(b => b.total_requests));

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      {/* Header */}
      <div>
        <div className="section-title">{t("page.stats", "使用统计")}</div>
        <div className="section-desc">{t("stats.desc", "按平台、分组、模型维度查看请求量与 Token 消耗趋势")}</div>
      </div>

      {/* Filters */}
      <div className="glass-surface" style={{ padding: "14px 20px", display: "flex", gap: 12, flexWrap: "wrap", alignItems: "center" }}>
        {/* Time preset */}
        <div style={{ display: "flex", gap: 4 }}>
          {(["today", "7d", "30d"] as TimePreset[]).map(p => (
            <button
              key={p}
              className={preset === p ? "btn-active" : "btn"}
              style={{ fontSize: 12, padding: "4px 10px" }}
              onClick={() => setPreset(p)}
            >
              {t(`stats.${p}`, p === "today" ? "今天" : p === "7d" ? "近 7 天" : "近 30 天")}
            </button>
          ))}
        </div>

        {/* Granularity */}
        <select className="input" style={{ fontSize: 12, width: 80 }} value={granularity} onChange={e => setGranularity(e.target.value as any)}>
          <option value="daily">{t("stats.daily", "按天")}</option>
          <option value="hourly">{t("stats.hourly", "按小时")}</option>
        </select>

        {/* Group by */}
        <select className="input" style={{ fontSize: 12, width: 100 }} value={groupBy} onChange={e => setGroupBy(e.target.value as any)}>
          <option value="platform">{t("stats.byPlatform", "按平台")}</option>
          <option value="model">{t("stats.byModel", "按模型")}</option>
          <option value="group">{t("stats.byGroup", "按分组")}</option>
        </select>

        {/* Filter: group */}
        <select className="input" style={{ fontSize: 12, width: 120 }} value={filterGroup} onChange={e => setFilterGroup(e.target.value)}>
          <option value="">{t("stats.allGroups", "全部分组")}</option>
          {groups.map(g => <option key={g.group.id} value={g.group.name}>{g.group.name}</option>)}
        </select>

        {/* Filter: model */}
        <select className="input" style={{ fontSize: 12, width: 150 }} value={filterModel} onChange={e => setFilterModel(e.target.value)}>
          <option value="">{t("stats.allModels", "全部模型")}</option>
          {allModels.map(m => <option key={m} value={m}>{m}</option>)}
        </select>

        {/* Filter: protocol */}
        <select className="input" style={{ fontSize: 12, width: 120 }} value={filterProtocol} onChange={e => setFilterProtocol(e.target.value)}>
          <option value="">{t("stats.allPlatforms", "全部平台")}</option>
          {Array.from(new Set(platforms.map(p => p.platform_type))).sort().map(p => <option key={p} value={p}>{p}</option>)}
        </select>
      </div>

      {loading && !data ? (
        <div style={{ textAlign: "center", padding: 40, color: "var(--text-secondary)", fontSize: F.hint }}>
          {t("stats.loading", "加载中...")}
        </div>
      ) : overview ? (
        <>
          {/* Overview cards */}
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
            {[
              { label: t("stats.totalRequests", "总请求"), value: formatNumber(overview.total_requests), unit: "" },
              { label: t("stats.successRate", "成功率"), value: overview.success_rate.toFixed(1), unit: "%" },
              { label: t("stats.inputTokens", "输入 Token"), value: formatNumber(overview.total_input_tokens), unit: "" },
              { label: t("stats.outputTokens", "输出 Token"), value: formatNumber(overview.total_output_tokens), unit: "" },
              { label: t("stats.cacheTokens", "缓存 Token"), value: formatNumber(overview.total_cache_tokens), unit: "" },
              { label: t("stats.cacheRate", "缓存率"), value: overview.cache_rate.toFixed(1), unit: "%" },
              { label: t("stats.avgLatency", "平均延迟"), value: overview.avg_duration_ms.toFixed(0), unit: "ms" },
            ].map((card, i) => (
              <div key={i} className="glass-surface" style={{ flex: "1 1 120px", padding: "16px 20px", display: "flex", flexDirection: "column", gap: 4 }}>
                <div style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{card.label}</div>
                <div style={{ fontSize: F.title, fontWeight: 700 }}>
                  {card.value}<span style={{ fontSize: F.label, fontWeight: 400, marginLeft: 2 }}>{card.unit}</span>
                </div>
              </div>
            ))}
          </div>

          {/* Trend chart */}
          {buckets.length > 0 && (
            <div className="glass-surface" style={{ padding: "16px 20px" }}>
              <div style={{ fontSize: F.label, fontWeight: 600, marginBottom: 12 }}>{t("stats.requestTrend", "请求趋势")}</div>
              <div style={{ display: "flex", alignItems: "flex-end", gap: 2, height: 120, overflow: "hidden" }}>
                {buckets.map((b, i) => {
                  const h = Math.max(2, (b.total_requests / maxReq) * 100);
                  const errRatio = b.total_requests > 0 ? b.error_count / b.total_requests : 0;
                  return (
                    <div key={i} title={`${b.time_bucket}: ${b.total_requests} requests`} style={{
                      flex: 1,
                      minWidth: 0,
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      gap: 2,
                    }}>
                      <div style={{ fontSize: 9, color: "var(--text-tertiary)" }}>{b.total_requests > 0 ? formatNumber(b.total_requests) : ""}</div>
                      <div style={{
                        width: "100%",
                        height: `${h}%`,
                        borderRadius: 3,
                        background: `linear-gradient(to top, var(--accent), color-mix(in srgb, var(--accent) ${Math.round(errRatio * 100)}%, var(--danger)))`,
                        opacity: 0.85,
                        transition: "height 0.3s",
                        position: "relative",
                      }} />
                      {buckets.length <= 24 && (
                        <div style={{ fontSize: 8, color: "var(--text-tertiary)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", maxWidth: "100%" }}>
                          {b.time_bucket.slice(-5)}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Dimension table */}
          {dims.length > 0 && (
            <div className="glass-surface" style={{ padding: "16px 20px" }}>
              <div style={{ fontSize: F.label, fontWeight: 600, marginBottom: 12 }}>
                {t("stats.dimensionRank", "维度排行")} — {t(`stats.by${groupBy.charAt(0).toUpperCase() + groupBy.slice(1)}`, groupBy)}
              </div>
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: F.hint }}>
                <thead>
                  <tr style={{ borderBottom: "1px solid var(--border)" }}>
                    <th style={{ textAlign: "left", padding: "6px 8px", fontWeight: 600 }}>{t("stats.dimName", "名称")}</th>
                    <th style={{ textAlign: "right", padding: "6px 8px" }}>{t("stats.requests", "请求")}</th>
                    <th style={{ textAlign: "right", padding: "6px 8px" }}>{t("stats.success", "成功")}</th>
                    <th style={{ textAlign: "right", padding: "6px 8px" }}>{t("stats.inputTokens", "输入")}</th>
                    <th style={{ textAlign: "right", padding: "6px 8px" }}>{t("stats.outputTokens", "输出")}</th>
                    <th style={{ textAlign: "right", padding: "6px 8px" }}>{t("stats.cacheTokens", "缓存")}</th>
                    <th style={{ textAlign: "right", padding: "6px 8px" }}>{t("stats.avgMs", "平均延迟")}</th>
                  </tr>
                </thead>
                <tbody>
                  {dims.map((d, i) => (
                    <tr key={i} style={{ borderBottom: "1px solid var(--border)", opacity: 0.9 }}>
                      <td style={{ padding: "6px 8px", fontWeight: 500, maxWidth: 200, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{d.name}</td>
                      <td style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.total_requests)}</td>
                      <td style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.success_count)}</td>
                      <td style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.input_tokens)}</td>
                      <td style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.output_tokens)}</td>
                      <td style={{ textAlign: "right", padding: "6px 8px" }}>{formatNumber(d.cache_tokens)}</td>
                      <td style={{ textAlign: "right", padding: "6px 8px" }}>{d.avg_duration_ms.toFixed(0)} ms</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      ) : (
        <div style={{ textAlign: "center", padding: 40, color: "var(--text-secondary)", fontSize: F.hint }}>
          {t("stats.noData", "暂无统计数据")}
        </div>
      )}
    </div>
  );
}
