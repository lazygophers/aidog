// GroupDetailPage — G3 分组详情页（设计稿「分组页终稿」1:1 独立成页）。
// 双栏观测台：中央统计带 + 桑基 flow + 成员表 + 模型映射 / 右候选序 + 最近路由 + 7 天趋势 + 组操作。
// 数据源全部复用现有 API；idiom 同 GroupEditPanel（页内独立视图态，Groups.tsx detailTarget 分支渲染）。
import { Fragment, useEffect, useMemo, useState } from "react";
import type { TFunction } from "i18next";
import type { GroupDetail, Platform, PlatformUsageStats } from "../../services/api";
import { groupFlowApi } from "../../services/api";
import { GroupSidePanels, staticOrder } from "./GroupSidePanels";
import type { CardsSnapshot } from "./GroupListItem";
import { GroupFlowPanel } from "./GroupFlowPanel";
import { LevelPriorityInput } from "../platforms/PlatformRow";
import { formatNumber, formatCost, formatPercent, successRate as calcSuccessRate } from "../../utils/formatters";
import { routingModeLabel } from "../../domains/groups";
import { Button } from "@/components/ui/button";

// 设计稿 token 映射：pgtop / pgbody / acard / seclab / 统计带 / memrow 全部按 6-整体风格与分组页.html。
const seclabStyle: React.CSSProperties = {
  fontSize: 10, color: "var(--text-tertiary)", fontFamily: "var(--font-mono, monospace)",
  letterSpacing: "1.2px", marginBottom: 8, textTransform: "uppercase",
};
const acardStyle: React.CSSProperties = {
  background: "var(--card)", border: "1px solid var(--border)", borderRadius: 10,
  padding: 12, boxShadow: "0 1px 3px rgba(28,36,48,.05)",
};

/** 中央统计带单卡（bigstat + statlab + delt）。 */
function StatCard(props: { big: React.ReactNode; label: string; delt?: React.ReactNode; deltUp?: boolean; children?: React.ReactNode }) {
  return (
    <div style={acardStyle}>
      <div style={{ fontSize: 28, fontWeight: 600, letterSpacing: "-.5px", fontVariantNumeric: "tabular-nums" }}>{props.big}</div>
      <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 2 }}>{props.label}</div>
      {props.delt != null && (
        <div style={{ fontSize: 11, marginTop: 4, color: props.deltUp ? "var(--color-success, #1F9D61)" : "var(--color-danger, #D14B4B)" }}>{props.delt}</div>
      )}
      {props.children}
    </div>
  );
}

/** 右栏 7 天趋势卡：daily API → 请求量 Top2 平台各一排柱。 */
function TrendCard(props: { groupKey: string; t: TFunction; platName: (id: number) => string }) {
  const [rows, setRows] = useState<{ day: string; platform_id: number; requests: number }[]>([]);
  useEffect(() => {
    let cancelled = false;
    groupFlowApi.daily(props.groupKey)
      .then(r => { if (!cancelled) setRows(r); })
      .catch(() => { if (!cancelled) setRows([]); });
    return () => { cancelled = true; };
  }, [props.groupKey]);
  const days = useMemo(() => [...new Set(rows.map(r => r.day))].sort(), [rows]);
  const platTotals = useMemo(() => {
    const m = new Map<number, number>();
    for (const r of rows) m.set(r.platform_id, (m.get(r.platform_id) ?? 0) + r.requests);
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  }, [rows]);
  const top = platTotals.slice(0, 2);
  const total7d = platTotals.reduce((s, [, v]) => s + v, 0);
  const maxDay = useMemo(() => {
    const m = new Map<string, number>();
    for (const r of rows) m.set(r.day, (m.get(r.day) ?? 0) + r.requests);
    return Math.max(1, ...m.values());
  }, [rows]);
  const chartColors = ["var(--chart-1)", "var(--chart-2)"];
  return (
    <div data-testid="detail-trend">
      <div style={seclabStyle}>7d</div>
      <div style={acardStyle} data-empty={rows.length === 0 ? "yes" : "no"}>
        {rows.length === 0 ? (
          <div style={{ fontSize: 11.5, color: "var(--text-tertiary)", padding: "10px 0" }}>{props.t("group.flow.empty", "窗口内无请求")}</div>
        ) : (
          <>
            {top.map(([pid], i) => (
              <div key={pid} style={{ display: "flex", gap: 4, alignItems: "flex-end", height: i === 0 ? 56 : 26, marginTop: i === 0 ? 0 : 2 }}>
                {days.map(day => {
                  const v = rows.find(r => r.day === day && r.platform_id === pid)?.requests ?? 0;
                  return (
                    <i key={day} title={props.platName(pid) + " · " + day + " · " + v}
                      style={{ flex: 1, height: Math.max(4, Math.round(v / maxDay * 100)) + "%", background: chartColors[i], borderRadius: "2px 2px 0 0", display: "block" }} />
                  );
                })}
              </div>
            ))}
            <div style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 9.5, marginTop: 6, color: "var(--text-tertiary)" }}>
              {top.map(([pid], i) => <span key={pid}>{i > 0 ? " " : ""}<span style={{ color: chartColors[i] }}>■</span> {props.platName(pid)}</span>)}
              {" · "}{formatNumber(total7d)}
            </div>
          </>
        )}
      </div>
    </div>
  );
}


/** 详情页成员行徽标：auto_disabled = 熔断冷却（设计稿 tg.d），disabled = 停用。 */
function MemberTag(props: { status: string; t: TFunction }) {
  if (props.status === "auto_disabled") {
    return <span style={{ fontSize: 9.5, borderRadius: 4, padding: "1px 6px", fontFamily: "var(--font-mono, monospace)", background: "rgba(209,75,75,.12)", color: "var(--color-danger, #D14B4B)" }}>{props.t("group.detail.broken", "熔断冷却")}</span>;
  }
  if (props.status === "disabled") {
    return <span style={{ fontSize: 9.5, borderRadius: 4, padding: "1px 6px", fontFamily: "var(--font-mono, monospace)", background: "rgba(199,126,30,.13)", color: "var(--color-warning, #C77E1E)" }}>{props.t("group.candidates.disabled", "停用")}</span>;
  }
  return null;
}

/** 中央成员表（设计稿 memrow 网格：状态点/名称/占比/成功率/费用/延迟/配额+操作）+ 底部模型映射。 */
function MembersCard(props: {
  detail: GroupDetail;
  platforms: Platform[];
  cards: CardsSnapshot;
  t: TFunction;
  onQuickTest: (p: Platform) => void;
  onEditPlatform: (p: Platform) => void;
  onSetLevelPriority?: (pid: number, v: number) => void;
  onDeleteMapping: (groupId: number, index: number) => void;
  onAddPlatform?: () => void;
  /** 成员行拖拽（复用列表页 usePlatformDrag：组内重排 priority 序，WKWebView pointer 方案） */
  onMemberPointerDown?: (ev: React.PointerEvent, pid: number, gid: number) => void;
  memberDropIndicator?: { gid: number; idx: number } | null;
}) {
  const { detail, cards, t } = props;
  const gps = detail.platforms;
  const fullPlats = gps
    .map(gp => props.platforms.find(pp => pp.id === gp.platform.id))
    .filter((pp): pp is Platform => !!pp);
  const totalReqs = fullPlats.reduce((s, p) => s + (cards.usageMap[p.id]?.total_requests ?? 0), 0);
  const statusColor = (s: string) =>
    s === "enabled" ? "var(--color-success, #1F9D61)" : s === "auto_disabled" ? "var(--color-danger, #D14B4B)" : "var(--text-tertiary)";
  return (
    <div style={{ ...acardStyle, padding: 0, overflow: "hidden" }} data-testid="detail-members" data-group-id={detail.group.id}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "10px 14px", borderBottom: "1px solid var(--border)" }}>
        <b style={{ fontSize: 12.5 }}>{t("group.detail.members", "成员平台")} · {fullPlats.length}</b>
        <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10.5, color: "var(--text-tertiary)" }}>{t("group.detail.byCandidateOrder", "按候选序")}</span>
        <div style={{ flex: 1 }} />
        {props.onAddPlatform && (
          <Button variant="outline" style={{ height: "auto", padding: "4px 10px", fontSize: 11 }} onClick={props.onAddPlatform}>
            {t("group.detail.addPlatform", "＋ 添加平台")}
          </Button>
        )}
      </div>
      {fullPlats.length === 0 ? (
        <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "18px 14px" }}>{t("group.empty", "暂无平台")}</div>
      ) : fullPlats.map((p, idx) => {
        const u = cards.usageMap[p.id];
        const share = totalReqs > 0 ? Math.round((u?.total_requests ?? 0) / totalReqs * 100) : 0;
        const sRate = u && u.total_requests > 0 ? calcSuccessRate(u.success_count, u.total_requests) : null;
        const last = cards.lastTestMap[p.id];
        const balance = cards.quotaMap[p.id]?.balance;
        const quotaPct = balance && balance.total ? Math.max(0, Math.min(100, Math.round(balance.remaining / balance.total * 100))) : null;
        const gp = gps.find(g => g.platform.id === p.id);
        const chartColors = ["var(--chart-1)", "var(--chart-2)", "var(--chart-3)", "var(--chart-4)"];
        return (
          <Fragment key={p.id}>
            {props.memberDropIndicator?.gid === detail.group.id && props.memberDropIndicator.idx === idx && (
              <div style={{ height: 2, background: "var(--accent)", borderRadius: 1, margin: "-1px 0", opacity: 0.7 }} />
            )}
          <div className="memrow" data-plat-id={p.id} data-gp-id={p.id} style={{
            display: "grid", gridTemplateColumns: "20px 22px 1fr 130px 74px 74px 84px 100px", gap: 8, alignItems: "center",
            padding: "10px 14px", borderBottom: "1px solid #EEF1F5", fontSize: 12,
            opacity: p.status === "enabled" ? 1 : 0.62,
          }}>
            {props.onMemberPointerDown ? (
              <span data-testid="detail-drag-handle" onPointerDown={(e) => props.onMemberPointerDown!(e, p.id, detail.group.id)}
                style={{ cursor: "grab", display: "inline-flex", alignItems: "center", touchAction: "none", color: "var(--text-tertiary)", opacity: 0.85 }}
                title={t("group.dragPlatform", "拖拽排序 / 移动到其他分组")}>
                <svg width="9" height="14" viewBox="0 0 10 16" fill="currentColor"><circle cx="3" cy="3" r="1.4" /><circle cx="7" cy="3" r="1.4" /><circle cx="3" cy="8" r="1.4" /><circle cx="7" cy="8" r="1.4" /><circle cx="3" cy="13" r="1.4" /><circle cx="7" cy="13" r="1.4" /></svg>
              </span>
            ) : <span />}
            <span style={{ width: 7, height: 7, borderRadius: "50%", background: statusColor(p.status), display: "inline-block" }} />
            <span style={{ fontWeight: 600, display: "flex", gap: 6, alignItems: "center", minWidth: 0 }}>
              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.platform_type}</span>
              {p.endpoints?.[0]?.protocol && <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10.5, color: "var(--text-tertiary)" }}>{p.endpoints[0].protocol}</span>}
              <MemberTag status={p.status} t={t} />
              {gp != null && <span title={t("group.candidates.posReason", "优先级 {{level}} / 序 {{prio}}", { level: gp.level_priority, prio: gp.priority })} style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10, color: "var(--text-tertiary)" }}>#{idx + 1}</span>}
              {/* per-group 候选优先级（1-10）行内直改，同列表页 LevelPriorityInput */}
              {gp != null && props.onSetLevelPriority && (
                <LevelPriorityInput testId="detail-row-level" title={t("platform.levelPriority", "候选优先级")}
                  value={gp.level_priority} onChange={(v) => props.onSetLevelPriority!(p.id, v)} />
              )}
            </span>
            <span>
              {formatNumber(u?.total_requests ?? 0)} · {share}%
              <span style={{ height: 5, borderRadius: 3, background: "var(--secondary, #EEF1F5)", overflow: "hidden", marginTop: 3, display: "block" }}>
                <i style={{ display: "block", height: "100%", width: share + "%", background: chartColors[idx % 4] }} />
              </span>
            </span>
            <span style={{ fontSize: 11.5, color: sRate == null ? "var(--text-tertiary)" : sRate >= 98 ? "var(--color-success, #1F9D61)" : sRate >= 90 ? "var(--color-warning, #C77E1E)" : "var(--color-danger, #D14B4B)" }}>
              {sRate == null ? "—" : formatPercent(sRate, 1)}
            </span>
            <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10.5, color: "var(--text-tertiary)" }}>${formatCost(u?.today_cost ?? 0)}</span>
            <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10.5, color: last ? (last.success ? "var(--text-tertiary)" : "var(--color-danger, #D14B4B)") : "var(--text-tertiary)" }}>
              {last ? (last.duration_ms + "ms " + (last.success ? "✓" : "✗")) : "—"}
            </span>
            <span style={{ display: "flex", gap: 6, alignItems: "center", justifyContent: "flex-end" }}>
              {quotaPct != null && (
                <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
                  <span style={{ width: 48, height: 5, borderRadius: 3, background: "var(--secondary, #EEF1F5)", overflow: "hidden", display: "inline-block" }}>
                    <i style={{ display: "block", height: "100%", width: quotaPct + "%", background: quotaPct > 20 ? "var(--primary)" : "var(--color-warning, #C77E1E)" }} />
                  </span>
                  <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10, color: "var(--text-tertiary)" }}>{quotaPct}%</span>
                </span>
              )}
              <Button variant="ghost" size="icon" style={{ width: 22, height: 22, minWidth: 22 }} onClick={() => props.onQuickTest(p)} title={t("group.detail.testOne", "快速测试")}>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M13 2 4 14h6l-1 8 9-12h-6l1-8z" /></svg>
              </Button>
              <Button variant="ghost" size="icon" style={{ width: 22, height: 22, minWidth: 22 }} onClick={() => props.onEditPlatform(p)} title={t("action.edit", "编辑")}>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>
              </Button>
            </span>
          </div>
          </Fragment>
        );
      })}
      {/* 模型映射（设计稿 mp 行：source chip → 平台 chip → 目标模型 chip） */}
      {detail.model_mappings.length > 0 && (
        <div style={{ borderTop: "1px solid var(--border)" }} data-testid="detail-mappings">
          <div style={{ padding: "8px 14px 4px", display: "flex", alignItems: "center", gap: 6 }}>
            <span style={{ ...seclabStyle, display: "inline", marginBottom: 0 }}>{t("group.detail.mappings", "模型映射")} · {detail.model_mappings.length}</span>
          </div>
          {detail.model_mappings.map((m, mi) => {
            const gp = gps.find(g => g.platform.id === m.target_platform_id);
            return (
              <div key={mi} style={{ display: "flex", alignItems: "center", gap: 8, padding: "7px 14px", fontSize: 12 }}>
                <span className="chip" style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10.5, border: "1px solid #DDC9F5", color: "var(--primary)", borderRadius: 6, padding: "2px 8px", background: "var(--accent-subtle, #FAFBFC)" }}>{m.source_model}</span>
                <span style={{ color: "var(--chart-2, #A377E8)" }}>⟶</span>
                {gp && <span className="chip" style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10.5, border: "1px solid var(--border)", borderRadius: 6, padding: "2px 8px", background: "var(--secondary, #FAFBFC)" }}>{gp.platform.platform_type}</span>}
                {gp && <span style={{ color: "var(--chart-2, #A377E8)" }}>⟶</span>}
                <span className="chip" style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10.5, border: "1px solid var(--border)", borderRadius: 6, padding: "2px 8px", background: "var(--secondary, #FAFBFC)", minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{m.target_model}</span>
                <div style={{ flex: 1 }} />
                <Button variant="ghost" size="icon" style={{ width: 22, height: 22, minWidth: 22 }} onClick={() => props.onDeleteMapping(detail.group.id, mi)} title={t("group.deleteMapping", "删除映射")}>
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"><path d="M2 2l6 6M8 2l-6 6" /></svg>
                </Button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}


/** 分组详情页（G3 设计稿「分组页终稿」1:1 三栏）。由 Groups.tsx detailTarget 分支渲染。 */
export function GroupDetailPage(props: {
  detail: GroupDetail;
  details: GroupDetail[];
  platforms: Platform[];
  t: TFunction;
  groupStats: Record<string, PlatformUsageStats>;
  cards: CardsSnapshot;
  cardService: import("../../components/platforms/PlatformCard").PlatformCardService;
  groupTestRunning: boolean;
  onSelectGroup: (id: number) => void;
  onClose: () => void;
  onEditGroup: (detail: GroupDetail) => void;
  onTestGroup: (group: GroupDetail["group"], gps: GroupDetail["platforms"]) => void;
  onEditPlatform: (p: Platform) => void;
  onSetLevelPriority: (groupId: number, pid: number, v: number) => void;
  onDeleteMapping: (groupId: number, index: number) => void;
  onAddPlatform?: () => void;
  /** 成员行拖拽（Groups.tsx 注入 usePlatformDrag 句柄） */
  onMemberPointerDown?: (ev: React.PointerEvent, pid: number, gid: number) => void;
  memberDropIndicator?: { gid: number; idx: number } | null;
}) {
  const { detail, details, platforms, t, groupStats, cards, cardService } = props;
  const { group } = detail;
  const gps = detail.platforms;
  const u = groupStats[group.group_key];
  const sRate = u ? calcSuccessRate(u.success_count, u.total_requests) : 0;
  const ordered = staticOrder(gps);
  const isLB = group.routing_mode === "load_balance";
  const platName = (id: number) => gps.find(g => g.platform.id === id)?.platform.platform_type ?? ("#" + id);
  const totalPlatLinks = details.reduce((s, d) => s + d.platforms.length, 0);
  return (
    <div data-testid="group-detail-page" style={{ display: "flex", flexDirection: "column", width: "100%", border: "1px solid var(--border)", borderRadius: 10, overflow: "hidden", background: "var(--card)", boxShadow: "0 1px 3px rgba(28,36,48,.05)" }}>
      {/* pgtop */}
      <div style={{ display: "flex", gap: 10, alignItems: "center", padding: "12px 16px", borderBottom: "1px solid var(--border)", background: "var(--secondary, #FAFBFC)" }}>
        <Button variant="ghost" size="icon" style={{ height: "auto" }} onClick={props.onClose} title={t("action.back", "返回")} data-testid="detail-back">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M19 12H5M12 19l-7-7 7-7" /></svg>
        </Button>
        <h1 style={{ fontSize: 15, fontWeight: 700, margin: 0 }}>{t("nav.groups", "分组")}</h1>
        {/* 当前组名必须上屏：详情页全页无组名时无法辨认展示的是哪个组 */}
        <span style={{ fontSize: 13, fontWeight: 600 }}>{group.name}</span>
        <span style={{ fontSize: 11.5, color: "var(--text-tertiary)" }}>{details.length} {t("group.detail.countGroups", "个分组")} · {totalPlatLinks} {t("group.detail.countLinks", "个平台关联")}</span>
        <div style={{ flex: 1 }} />
      </div>
      {/* pgbody：1fr / 264px */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 264px", gap: 14, padding: "14px 16px", background: "linear-gradient(180deg, var(--secondary, #FBFBFD), transparent)", alignItems: "start" }}>
        {/* 中央：统计带 + flow + 成员表 */}
        <div style={{ display: "flex", flexDirection: "column", gap: 12, minWidth: 0 }}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 10 }} data-testid="detail-stats">
            <StatCard big={formatNumber(u?.today_tokens ?? 0)} label={t("group.detail.todayTokens", "今日 tokens")} />
            <StatCard big={"$" + formatCost(u?.today_cost ?? 0)} label={t("group.stats.todayCost", "今日费用")} />
            <StatCard big={<>{u && u.total_requests > 0 ? formatPercent(sRate, 1) : "—"}{u && u.total_requests > 0 ? <em style={{ fontStyle: "normal", fontSize: 13, color: "var(--text-tertiary)", fontWeight: 400 }}>%</em> : null}</>}
              label={t("group.detail.successRate", "成功率")}
              delt={u && u.total_requests > 0 ? (u.total_requests - u.success_count) + " " + t("group.detail.failedCount", "次失败") : undefined}
              deltUp={false} />
            <div style={{ ...acardStyle, borderColor: "var(--primary)", background: "linear-gradient(150deg, var(--accent-subtle, #FBF9FE), var(--card))" }} data-testid="detail-next-request">
              <div style={{ fontSize: 10, letterSpacing: "1.2px", color: "var(--text-tertiary)", fontFamily: "var(--font-mono, monospace)" }}>
                {isLB ? t("group.next.lbTitle", "下一个请求 · 按权重") : t("group.next.title", "下一个请求")}
              </div>
              <div style={{ fontSize: 15, fontWeight: 700, marginTop: 5 }}>
                {ordered.length === 0
                  ? <span style={{ fontSize: 11.5, color: "var(--text-tertiary)" }}>{t("group.candidates.none", "无启用中的候选平台")}</span>
                  : isLB
                    ? t("group.next.distributed", { defaultValue: "按权重分布（{{n}} 个候选）", n: ordered.length })
                    : <>→ <b style={{ color: "var(--primary)" }}>{ordered[0].platform.platform_type}</b></>}
              </div>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 4, marginTop: 8 }}>
                {ordered.map((g, i) => (
                  <span key={g.platform.id} style={{
                    fontSize: 10.5, border: "1px solid " + (i === 0 && !isLB ? "var(--primary)" : "var(--border)"), borderRadius: 99,
                    padding: "2px 9px", color: i === 0 && !isLB ? "var(--primary)" : "var(--text-tertiary)",
                    fontWeight: i === 0 && !isLB ? 600 : 400, background: i === 0 && !isLB ? "var(--accent-subtle)" : "transparent",
                  }}>
                    #{i + 1} {g.platform.platform_type}{i === 0 && !isLB ? " ✓" : ""}
                  </span>
                ))}
              </div>
              <div style={{ fontSize: 10.5, color: "var(--text-tertiary)", marginTop: 7 }}>{routingModeLabel(t, group.routing_mode)}</div>
            </div>
          </div>
          <div style={{ border: "1px solid var(--border)", borderRadius: 10, background: "var(--card)" }} data-testid="detail-flow">
            <GroupFlowPanel groupKey={group.group_key} gps={gps} />
          </div>
          <MembersCard
            detail={detail} platforms={platforms} cards={cards} t={t}
            onQuickTest={cardService.onQuickTest}
            onEditPlatform={props.onEditPlatform}
            onDeleteMapping={props.onDeleteMapping}
            onAddPlatform={props.onAddPlatform}
            onSetLevelPriority={(pid, v) => props.onSetLevelPriority(group.id, pid, v)}
            onMemberPointerDown={props.onMemberPointerDown}
            memberDropIndicator={props.memberDropIndicator}
          />
        </div>
        {/* 右：候选序 + 最近路由 + 趋势 + 组操作 */}
        <div>
          <GroupSidePanels group={group} gps={gps} />
          <TrendCard groupKey={group.group_key} t={t} platName={platName} />
          <div style={{ marginTop: 14 }}>
            <div style={seclabStyle}>{t("group.detail.groupOps", "组操作")}</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <Button variant="outline" style={{ justifyContent: "flex-start", height: "auto", padding: "6px 10px", fontSize: 12 }}
                disabled={gps.filter(gp => gp.platform.status === "enabled").length === 0 || props.groupTestRunning}
                onClick={() => props.onTestGroup(group, gps)}>
                {t("group.testAll", "一键测试本组")}
              </Button>
              <Button variant="outline" style={{ justifyContent: "flex-start", height: "auto", padding: "6px 10px", fontSize: 12 }}
                onClick={() => props.onEditGroup(detail)}>
                {t("action.edit", "编辑")}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
