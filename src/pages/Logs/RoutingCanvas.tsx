import { useLayoutEffect, useMemo, useRef, useState } from "react";
import type { TFunction } from "i18next";
import type { RouteReason } from "../../services/api/types/generated/RouteReason";
import type { ModelVia } from "../../services/api/types/generated/ModelVia";
import type { RouteTag } from "../../services/api/types/generated/RouteTag";
import type { RoutingSnapshot } from "../../services/api/types/generated/RoutingSnapshot";
import type { RouteCandidate } from "../../services/api/types/generated/RouteCandidate";
import type { ShortcutBypass } from "../../services/api/types/generated/ShortcutBypass";
import { F } from "../../domains/shared/tokens";
import { formatDurationMs, formatMoney, formatNumber } from "../../utils/formatters";
import { useDisplayCurrency } from "../../utils/displayCurrency";

/** 请求详情「路由决策」画布：按网关真实执行顺序分五列（入站 / 路由判定 / 候选筛选与排序 + 逐平台转发 / 响应）。
 *  设计稿 .scratch/路由画布-完整流程/design-demos/设计稿.html（v2）。节点 id 稳定：req / route / c-{平台} / s-{平台}-{序} / resp。 */

type T = TFunction<"translation", undefined>;

// 数据驱动 key 映射（t(变量) 对 check-i18n 静态扫描不可见，词表由类型锁死）
const REASON_KEY: Record<RouteReason, string> = {
  "selected": "logs.routing.reason.selected",
  "disabled-manual": "logs.routing.reason.disabled-manual",
  "disabled-auto": "logs.routing.reason.disabled-auto",
  "peak-disabled": "logs.routing.reason.peak-disabled",
  "breaker": "logs.routing.reason.breaker",
  "quota-cooldown": "logs.routing.reason.quota-cooldown",
  "auth-cooldown": "logs.routing.reason.auth-cooldown",
  "unknown-protocol": "logs.routing.reason.unknown-protocol",
  "kind-unsupported": "logs.routing.reason.kind-unsupported",
  "client-not-allowed": "logs.routing.reason.client-not-allowed",
  "rate-limited": "logs.routing.reason.rate-limited",
  "model-not-on-platform": "logs.routing.reason.model-not-on-platform",
  "skipped-dead-upstream": "logs.routing.reason.skipped-dead-upstream",
  "not-declared": "logs.routing.reason.not-declared",
  "retry-limit": "logs.routing.reason.retry-limit",
};
const TAG_KEY: Record<RouteTag, string> = {
  "mapping-promoted": "logs.routing.tag.mapping-promoted",
  "coding-plan": "logs.routing.tag.coding-plan",
  "demoted": "logs.routing.tag.demoted",
  "sticky": "logs.routing.tag.sticky",
  "probe": "logs.routing.tag.probe",
  "breaker-backfill": "logs.routing.tag.breaker-backfill",
};
const MODE_KEY: Record<string, string> = {
  load_balance: "logs.routing.mode.load_balance",
  failover: "logs.routing.mode.failover",
  health_aware: "logs.routing.mode.health_aware",
  sticky: "logs.routing.mode.sticky",
};
const VIA_KEY: Record<ModelVia, string> = {
  "mapped": "logs.routing.via.mapped",
  "time-window": "logs.routing.via.time-window",
  "preset-peak": "logs.routing.via.preset-peak",
  "slot-opus": "logs.routing.via.slot-opus",
  "slot-sonnet": "logs.routing.via.slot-sonnet",
  "slot-haiku": "logs.routing.via.slot-haiku",
  "slot-gpt": "logs.routing.via.slot-gpt",
  "slot-default": "logs.routing.via.slot-default",
  "dedicated-slot": "logs.routing.via.dedicated-slot",
  "passthrough": "logs.routing.via.passthrough",
};
const BYPASS_KEY: Record<ShortcutBypass, string> = {
  "auto-disabled": "logs.routing.bypass.auto-disabled",
  "expired": "logs.routing.bypass.expired",
  "breaker": "logs.routing.bypass.breaker",
  "quota-cooldown": "logs.routing.bypass.quota-cooldown",
  "auth-cooldown": "logs.routing.bypass.auth-cooldown",
};
// 尝试种类 / 错误类别 / 副作用 / 链停止原因：后端写字符串，未知值回落原文
const KIND_KEY: Record<string, string> = {
  retry: "logs.routing.kind.retry",
  beta_downgrade: "logs.routing.kind.beta_downgrade",
  empty_200: "logs.routing.kind.empty_200",
  rate_limit_platform: "logs.routing.kind.rate_limit_platform",
  rate_limit_model: "logs.routing.kind.rate_limit_model",
};
const CLASS_KEY: Record<string, string> = {
  model: "logs.routing.class.model",
  service: "logs.routing.class.service",
  unknown: "logs.routing.class.unknown",
};
const EFFECT_KEY: Record<string, string> = {
  breaker_count: "logs.routing.effect.breaker_count",
  penalty_server: "logs.routing.effect.penalty_server",
  penalty_client: "logs.routing.effect.penalty_client",
  cooldown_minute: "logs.routing.effect.cooldown_minute",
  cooldown_quota: "logs.routing.effect.cooldown_quota",
  cooldown_balance: "logs.routing.effect.cooldown_balance",
  cooldown_auth: "logs.routing.effect.cooldown_auth",
  disabled_censorship: "logs.routing.effect.disabled_censorship",
};
const STOP_KEY: Record<string, string> = {
  service: "logs.routing.chain.skip.service",
  rule: "logs.routing.chain.skip.rule",
  rate_limit: "logs.routing.chain.skip.rate_limit",
};
const EDGE_KEY = {
  "next-model": "logs.routing.edge.next-model",
  "retry": "logs.routing.edge.retry",
  "switch": "logs.routing.edge.switch",
  "switch-service": "logs.routing.edge.switch-service",
  "switch-model": "logs.routing.edge.switch-model",
  "switch-unknown": "logs.routing.edge.switch-unknown",
  "switch-rule": "logs.routing.edge.switch-rule",
  "switch-rate_limit": "logs.routing.edge.switch-rate_limit",
  "exhausted": "logs.routing.edge.exhausted",
} as const;
type EdgeLabel = keyof typeof EDGE_KEY;
const WHY_KEY = {
  "already-succeeded": "logs.routing.why.already-succeeded",
  "untried": "logs.routing.why.untried",
} as const;

export type AttemptLite = {
  platform_id: number; status_code: number; model?: string; error?: string; chain_stop?: string; duration_ms?: number; ttfb_ms?: number;
  kind?: string; error_class?: string; code_meaning?: string; error_rule?: string; effects?: string[];
};

type EdgeKind = "eval" | "ok" | "bad" | "out";
interface Edge { from: string; to: string; kind: EdgeKind; label?: EdgeLabel; why?: string }
type StepState = "ok" | "bad" | "skip" | "untried";
interface Step {
  id: string; state: StepState; model?: string; attempt?: AttemptLite; stop?: string; next?: EdgeLabel;
  /** 限频跳过（kind=rate_limit_*，后端 status_code=0）：按跳过格画，不是连接失败。连线逻辑仍按 state（bad）走。 */
  rateSkip?: boolean;
}
type LaneState = "win" | "fail" | "dim";
interface Lane {
  id: string; entry: RouteCandidate; state: LaneState; steps: Step[];
  /** 未选原因：RouteReason（非 selected）或 already-succeeded / untried；入选且尝试过则 undefined。 */
  notChosen?: string;
}

/** 成功 = 2xx 且无 error 且非空 200。决策 B（200 空响应）的 attempt 记 status_code=200 + 非空 error（kind=empty_200），须算失败。 */
const isRateSkip = (a?: AttemptLite) => !!a?.kind?.startsWith("rate_limit");
const attemptOk = (a: AttemptLite) => a.status_code >= 200 && a.status_code < 300 && !a.error && a.kind !== "empty_200";

/** 换平台连线标签：链停止原因优先，其次错误类别。 */
function switchLabel(s: Step): EdgeLabel {
  const a = s.attempt;
  const stop = s.stop;
  if (stop === "rule") return "switch-rule";
  if (stop === "rate_limit") return "switch-rate_limit";
  if (a?.error_class === "service" || a?.error_class === "model" || a?.error_class === "unknown") return `switch-${a.error_class}` as EdgeLabel;
  return "switch";
}

/** 路由快照 + 尝试 → 候选行（候选 + 其全部尝试）与连线集合。纯数据，无几何。 */
function buildFlow(routing: RoutingSnapshot, attempts: AttemptLite[] | undefined, statusOk: boolean) {
  const all = attempts ?? [];
  const wins = all.filter(attemptOk);
  const winPid = wins.length ? wins[wins.length - 1]!.platform_id : null;
  const lanes: Lane[] = [];
  const edges: Edge[] = [{ from: "req", to: "route", kind: "eval" }];
  let prev: Step | null = null;
  let winStep: Step | null = null;
  for (const e of routing.entries) {
    const pid = e.platform_id;
    const mine = all.filter((a) => a.platform_id === pid);
    const lane: Lane = { id: `c-${pid}`, entry: e, state: "dim", steps: [] };
    if (e.reason !== "selected") {
      lane.notChosen = e.reason;
    } else if (mine.length === 0) {
      lane.notChosen = winPid !== null ? "already-succeeded" : "untried";
      const chain = e.model_chain ?? [];
      if (chain.length > 1) lane.steps = chain.map((m, i) => ({ id: `s-${pid}-${i}`, state: "untried" as const, model: m }));
    } else {
      lane.steps = mine.map((a, i) => ({ id: `s-${pid}-${i}`, state: attemptOk(a) ? "ok" : "bad", model: a.model, attempt: a, stop: a.chain_stop || undefined, rateSkip: isRateSkip(a) }));
      // 链上没轮到的模型：链停止（限频 / 服务类 / 规则）→ 跳过格；否则（前面已成功）→ 未轮到格
      const lastA = mine[mine.length - 1]!;
      const chain = e.model_chain ?? [];
      if (chain.length > 1) {
        const tried = new Set(mine.map((a) => a.model));
        for (const m of chain.filter((m) => !tried.has(m))) {
          lane.steps.push(lastA.chain_stop
            ? { id: `s-${pid}-${lane.steps.length}`, state: "skip", model: m, stop: lastA.chain_stop }
            : { id: `s-${pid}-${lane.steps.length}`, state: "untried", model: m });
        }
      }
      lane.state = pid === winPid ? "win" : "fail";
    }
    lanes.push(lane);

    if (!lane.steps.some((s) => s.state === "ok" || s.state === "bad")) {
      edges.push({ from: "route", to: lane.id, kind: "out", why: lane.notChosen });
      continue;
    }
    edges.push(prev ? { from: prev.id, to: lane.id, kind: "bad", label: switchLabel(prev) } : { from: "route", to: lane.id, kind: "eval" });
    edges.push({ from: lane.id, to: lane.steps[0]!.id, kind: "eval" });
    lane.steps.forEach((s, i) => {
      if (i === 0) return;
      const p = lane.steps[i - 1]!;
      if (s.state === "skip" || s.state === "untried") { edges.push({ from: p.id, to: s.id, kind: "out" }); return; }
      const label: EdgeLabel = p.attempt?.kind === "retry" || p.attempt?.kind === "beta_downgrade" ? "retry" : "next-model";
      p.next = label;
      edges.push({ from: p.id, to: s.id, kind: "bad", label });
    });
    const tried = lane.steps.filter((s) => s.state === "ok" || s.state === "bad");
    const lastTried = tried[tried.length - 1]!;
    if (lane.state === "win") winStep = lastTried; else prev = lastTried;
  }
  if (winStep) edges.push({ from: winStep.id, to: "resp", kind: "ok" });
  else if (prev) edges.push({ from: prev.id, to: "resp", kind: "bad", label: "exhausted" });
  else edges.push({ from: "route", to: "resp", kind: statusOk ? "eval" : "bad", label: statusOk ? undefined : "exhausted" });
  return { lanes, edges, winStep };
}

type CheckState = "ok" | "warn" | "na" | "unrec" | "bad" | "inferred";
const MARK: Record<CheckState, string> = { ok: "✓", warn: "↷", na: "—", unrec: "?", bad: "✕", inferred: "≈" };

function Check({ id, state, label, t, children }: { id: string; state: CheckState; label: string; t: T; children?: React.ReactNode }) {
  return (
    <li data-check={id} data-state={state}>
      <span className="rc-k">{label}</span>
      <span className={`rc-mark ${state}`} aria-hidden="true">{MARK[state]}</span>
      <span className="rc-v">{state === "unrec" ? <span className="rc-unrec" data-testid="routing-unrecorded">{t("logs.routing.unrecorded")}</span> : children}</span>
    </li>
  );
}

const STATE_ICON: Record<string, string> = { win: "✓", fail: "✕", dim: "·" };

export function RoutingCanvas({
  routing, attempts, platformMap, durationMs, statusCode, actualModel, requestModel, t,
  groupName, client, inputTokens, outputTokens, estCost, retryCount, blockedBy, blockedReason,
}: {
  routing: RoutingSnapshot | null;
  attempts: AttemptLite[] | undefined;
  platformMap: Map<number, string>;
  durationMs: number;
  statusCode: number;
  actualModel?: string;
  requestModel?: string;
  t: T;
  groupName?: string;
  client?: string;
  inputTokens?: number;
  outputTokens?: number;
  estCost?: number;
  retryCount?: number;
  blockedBy?: string;
  blockedReason?: string;
}) {
  useDisplayCurrency();
  const wrapRef = useRef<HTMLDivElement>(null);
  const flowRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [paths, setPaths] = useState<Record<string, { d: string; lx: number; ly: number }>>({});

  const statusOk = statusCode >= 200 && statusCode < 300;
  const flow = useMemo(
    () => (routing && routing.entries.length > 0 ? buildFlow(routing, attempts, statusOk) : null),
    [routing, attempts, statusOk],
  );
  const vertical = width > 0 && width < 720;

  // 容器宽度（ResizeObserver，80ms debounce；jsdom 无 RO 时只量一次）
  useLayoutEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    setWidth(el.clientWidth);
    if (typeof ResizeObserver === "undefined") return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ro = new ResizeObserver(() => { clearTimeout(timer); timer = setTimeout(() => setWidth(el.clientWidth), 80); });
    ro.observe(el);
    return () => { ro.disconnect(); clearTimeout(timer); };
  }, [flow !== null]); // eslint-disable-line react-hooks/exhaustive-deps

  // 连线几何：节点实测矩形 → 贝塞尔。宽布局向右走水平线、换平台（向下回绕）走纵向线；窄屏沿左侧槽道绕行。
  useLayoutEffect(() => {
    const root = flowRef.current;
    if (!root || !flow) return;
    const box = root.getBoundingClientRect();
    const rect = (id: string) => root.querySelector(`[data-node-id="${id}"]`)?.getBoundingClientRect();
    const out: Record<string, { d: string; lx: number; ly: number }> = {};
    flow.edges.forEach((e, i) => {
      const A = rect(e.from); const B = rect(e.to);
      if (!A || !B) return;
      const ax = A.left - box.left, ay = A.top - box.top, bx = B.left - box.left, by = B.top - box.top;
      let d: string, lx: number, ly: number;
      if (vertical) {
        const g = Math.max(2, Math.min(ax, bx) - 8 - (i % 4) * 4);
        const y1 = ay + A.height / 2, y2 = by + B.height / 2;
        d = `M${ax},${y1} C${g},${y1} ${g},${y2} ${bx},${y2}`;
        lx = g; ly = (y1 + y2) / 2;
      } else if (bx >= ax + A.width - 1) {
        const x1 = ax + A.width, y1 = ay + A.height / 2, y2 = by + B.height / 2, mx = (x1 + bx) / 2;
        d = `M${x1},${y1} C${mx},${y1} ${mx},${y2} ${bx},${y2}`;
        lx = mx; ly = (y1 + y2) / 2 - 8;
      } else {
        const x1 = ax + A.width / 2, y1 = ay + A.height, x2 = bx + B.width / 2;
        d = `M${x1},${y1} C${x1},${y1 + 40} ${x2},${by - 40} ${x2},${by}`;
        lx = (x1 + x2) / 2; ly = (y1 + by) / 2 - 4;
      }
      out[`${e.from}->${e.to}`] = { d, lx, ly };
    });
    setPaths(out);
  }, [flow, vertical, width, attempts]);

  const modeLabel = routing?.mode != null && MODE_KEY[routing.mode] !== undefined ? t(MODE_KEY[routing.mode]!) : null;

  // ── 入站 / 路由判定 检查项（旧快照缺字段 → unrec）──
  const entries = routing?.entries ?? [];
  const newFormat = routing ? routing.max_retries !== undefined || routing.shortcut !== undefined || routing.eval_ms !== undefined : false;
  const viaAny = (v: ModelVia) => entries.some((c) => (c.model_via ?? []).includes(v));
  const backfillN = entries.filter((c) => (c.tags ?? []).includes("breaker-backfill")).length;
  const clientBlocked = entries.filter((c) => c.reason === "client-not-allowed").length;
  const sc = routing?.shortcut;
  const winTtfb = flow?.winStep?.attempt?.ttfb_ms;

  const stepText = (s: Step) => {
    const a = s.attempt;
    if (s.state === "skip" || s.state === "untried" || s.rateSkip) return null;
    return a!.status_code === 0 ? t("logs.routing.step.connectFailed") : String(a!.status_code);
  };

  return (
    <div className="glass-surface hover-lift" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 10 }} data-testid="routing-canvas">
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span style={{ fontSize: F.body, fontWeight: 600 }}>{t("logs.routingDecision", "路由决策")}</span>
        {modeLabel && <span className="rc-pill primary" data-testid="routing-mode">{modeLabel}</span>}
      </div>
      {!flow || !routing ? (
        <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }} data-testid="routing-empty">
          {t("logs.routing.noSnapshot", "无路由决策记录")}
        </span>
      ) : (
        <div ref={wrapRef} className="rc-scroll">
          <div ref={flowRef} className="rc-flow" data-testid="routing-flow" data-layout={vertical ? "vertical" : "wide"}>
            {/* ① 入站 */}
            <div className="rc-col">
              <div className="rc-col-title">{t("logs.routing.col.inbound")}</div>
              <div className="rc-node" data-node-id="req" data-state="dim">
                <h3><span className="rc-ico">→</span>{t("logs.routing.request", "请求")}</h3>
                {requestModel && <div className="rc-sub mono" data-testid="routing-request-model">{requestModel}{client ? ` · ${client}` : ""}</div>}
                {groupName && <div className="rc-sub" data-testid="routing-group">{t("logs.routing.group")} <b>{groupName}</b></div>}
                <ul className="rc-checks">
                  <Check id="gate" state="inferred" label={t("logs.routing.check.gate")} t={t}>{t("logs.routing.val.inferredPass")}</Check>
                  <Check id="clientGate" state={clientBlocked > 0 ? "warn" : "ok"} label={t("logs.routing.check.clientGate")} t={t}>
                    {clientBlocked > 0 ? `${t("logs.routing.val.excluded")} ${clientBlocked}` : t("logs.routing.val.pass")}
                  </Check>
                </ul>
              </div>
            </div>
            {/* ② 路由判定 */}
            <div className="rc-col">
              <div className="rc-col-title">{t("logs.routing.col.route")}</div>
              <div className="rc-node" data-node-id="route" data-state="dim">
                <h3><span className="rc-ico">⇄</span>{t("logs.routing.route", "路由")}</h3>
                {routing.eval_ms !== undefined && <div className="rc-sub mono" data-testid="routing-eval-ms" data-ms={routing.eval_ms}>{formatDurationMs(routing.eval_ms)}</div>}
                <ul className="rc-checks">
                  <Check id="mapping" state={viaAny("mapped") ? "ok" : "na"} label={t("logs.routing.check.mapping")} t={t}>
                    {viaAny("mapped") ? t("logs.routing.val.hit") : t("logs.routing.val.miss")}
                  </Check>
                  <Check id="direct" state={viaAny("passthrough") || entries.some((c) => c.reason === "not-declared") ? "ok" : "na"} label={t("logs.routing.check.direct")} t={t}>
                    {viaAny("passthrough") || entries.some((c) => c.reason === "not-declared") ? t("logs.routing.val.hit") : t("logs.routing.val.miss")}
                  </Check>
                  <Check id="shortcut" state={sc ? "warn" : newFormat ? "na" : "unrec"} label={t("logs.routing.check.shortcut")} t={t}>
                    {sc ? (
                      <span data-literal={String(sc.literal)}>
                        {t(sc.literal ? "logs.routing.val.literal" : "logs.routing.val.equivalent")}
                        {(sc.bypassed ?? []).map((b) => (
                          <span key={b} className="rc-pill warn" data-testid="routing-bypass" data-bypass={b}>{t(BYPASS_KEY[b])}</span>
                        ))}
                      </span>
                    ) : t("logs.routing.val.notTriggered")}
                  </Check>
                  <Check id="backfill" state={backfillN > 0 ? "warn" : newFormat ? "na" : "unrec"} label={t("logs.routing.check.backfill")} t={t}>
                    {backfillN > 0 ? `${t("logs.routing.val.backfilled")} ${backfillN}` : t("logs.routing.val.notTriggered")}
                  </Check>
                  <Check id="maxRetries" state={routing.max_retries !== undefined ? "na" : "unrec"} label={t("logs.routing.check.maxRetries")} t={t}>
                    <span data-value={routing.max_retries}>{routing.max_retries}</span>
                  </Check>
                </ul>
              </div>
            </div>
            {/* ③ 候选筛选与排序 + ④ 逐平台转发 */}
            <div className="rc-col">
              <div className="rc-col-title">{t("logs.routing.col.flow")}</div>
              <div className="rc-matrix">
                {flow.lanes.map((ln) => {
                  const e = ln.entry;
                  const excluded = e.reason !== "selected";
                  const name = platformMap.get(e.platform_id) || e.platform || `#${e.platform_id}`;
                  return (
                    <div className="rc-row" key={ln.id}>
                      <div className={`rc-node rc-cand ${excluded ? "out" : ""}`} data-node-id={ln.id} data-state={ln.state}>
                        <div className="rc-cand-head">
                          <span className="rc-ico">{STATE_ICON[ln.state]}</span>
                          <span className="rc-name">{name}</span>
                          {!excluded && (
                            <span className="rc-ord mono" data-testid="routing-order" data-order={e.order}>#{e.order} · P{e.level_priority}·{e.priority}</span>
                          )}
                        </div>
                        <div className="rc-tags">
                          {(e.tags ?? []).map((tg) => <span key={tg} className="rc-pill" data-testid="routing-tag" data-tag={tg}>{t(TAG_KEY[tg])}</span>)}
                          {(e.model_via ?? []).map((v) => <span key={v} className="rc-pill" data-testid="routing-model-via" data-via={v}>{t(VIA_KEY[v])}</span>)}
                          {e.target_model && !excluded && <span className="rc-sub mono" data-testid="routing-target-model">{e.target_model}</span>}
                        </div>
                        {ln.notChosen && (
                          <span className={`rc-pill ${excluded ? "bad" : ""}`} data-testid="routing-not-chosen" data-reason={ln.notChosen}>
                            {t("logs.routing.notChosen")}
                            {ln.notChosen in WHY_KEY ? t(WHY_KEY[ln.notChosen as keyof typeof WHY_KEY]) : t(REASON_KEY[ln.notChosen as RouteReason])}
                          </span>
                        )}
                      </div>
                      <div className="rc-steps">
                        {ln.steps.map((s) => {
                          const a = s.attempt;
                          const shown = s.rateSkip ? "skip" : s.state;
                          const failed = shown === "bad";
                          return (
                            <div key={s.id} className={`rc-node rc-step ${shown}`} data-node-id={s.id} data-state={shown} data-kind={a?.kind ?? "normal"}>
                              <span className="mono rc-step-model" data-testid="routing-step-model">{s.model ?? e.target_model ?? actualModel ?? "-"}</span>
                              {a && (
                                <span className="rc-step-line">
                                  {!s.rateSkip && <span data-testid="routing-step-status">{stepText(s)}</span>}
                                  {a.kind && <span className="rc-pill" data-testid="routing-step-kind" data-kind={a.kind}>{t(KIND_KEY[a.kind] ?? a.kind)}</span>}
                                  {failed && a.error_class && <span className="rc-pill bad" data-testid="routing-step-class" data-class={a.error_class}>{t(CLASS_KEY[a.error_class] ?? a.error_class)}</span>}
                                  {failed && !a.kind && !a.error_class && <span className="rc-unrec" data-testid="routing-unrecorded">{t("logs.routing.unrecorded")}</span>}
                                  {a.duration_ms !== undefined && <span className="rc-sub" data-testid="routing-step-ms" data-ms={a.duration_ms}>{formatDurationMs(a.duration_ms)}</span>}
                                </span>
                              )}
                              {a?.code_meaning && <span className="rc-sub" data-testid="routing-step-meaning">{a.code_meaning}</span>}
                              {a?.error_rule && <span className="rc-sub" data-testid="routing-step-rule">{a.error_rule}</span>}
                              {a?.effects && a.effects.length > 0 && (
                                <span className="rc-tags">
                                  {a.effects.map((ef) => <span key={ef} className="rc-pill warn" data-testid="routing-step-effect" data-effect={ef}>{t(EFFECT_KEY[ef] ?? ef)}</span>)}
                                </span>
                              )}
                              {s.state === "untried" && <span className="rc-sub" data-testid="routing-step-untried">{t("logs.routing.chain.untried")}</span>}
                              {s.stop && <span className="rc-sub" data-testid="routing-step-stop" data-stop={s.stop}>{t(STOP_KEY[s.stop] ?? s.stop)}</span>}
                              {s.next && !s.stop && <span className="rc-sub" data-testid="routing-step-next" data-next={s.next}>{t(EDGE_KEY[s.next])}</span>}
                              {a && a.ttfb_ms !== undefined && s.state === "ok" && <span className="rc-sub" data-testid="routing-step-ttfb" data-ms={a.ttfb_ms}>{t("logs.routing.ttfb", "首字")} {formatDurationMs(a.ttfb_ms)}</span>}
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
            {/* ⑤ 响应 */}
            <div className="rc-col">
              <div className="rc-col-title">{t("logs.routing.col.resp")}</div>
              <div className="rc-node" data-node-id="resp" data-state={statusOk ? "win" : "fail"}>
                <h3>
                  <span className="rc-ico">{statusOk ? "✓" : "✕"}</span>
                  <span data-testid="routing-resp-status" data-value={statusCode}>{statusCode}</span>
                </h3>
                <div className="rc-sub mono" data-testid="routing-resp-duration" data-ms={durationMs}>{formatDurationMs(durationMs)}</div>
                {winTtfb !== undefined && <div className="rc-sub mono" data-testid="routing-resp-ttfb" data-ms={winTtfb}>{t("logs.routing.ttfb", "首字")} {formatDurationMs(winTtfb)}</div>}
                {inputTokens !== undefined && outputTokens !== undefined && (
                  <div className="rc-sub mono" data-testid="routing-resp-tokens" data-in={inputTokens} data-out={outputTokens}>
                    {t("logs.routing.resp.tokens")} {formatNumber(inputTokens)} / {formatNumber(outputTokens)}
                  </div>
                )}
                {estCost !== undefined && <div className="rc-sub mono" data-testid="routing-resp-cost" data-value={estCost}>{formatMoney(estCost)}</div>}
                {retryCount !== undefined && <div className="rc-sub mono" data-testid="routing-resp-retry" data-value={retryCount}>{t("logs.routing.resp.retry")} {retryCount}</div>}
                {(blockedBy || blockedReason) && (
                  <div className="rc-sub mono" data-testid="routing-resp-blocked" data-by={blockedBy ?? ""} data-reason={blockedReason ?? ""}>
                    {blockedBy}{blockedBy && blockedReason ? " · " : ""}{blockedReason}
                  </div>
                )}
              </div>
            </div>
            <svg className="rc-edges" aria-hidden="true">
              <defs>
                {(["ok", "bad", "out", "eval"] as const).map((k) => (
                  <marker key={k} id={`rc-ar-${k}`} viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
                    <path className={`rc-ar ${k}`} d="M0,0 L8,4 L0,8z" />
                  </marker>
                ))}
              </defs>
              {flow.edges.map((e) => {
                const key = `${e.from}->${e.to}`;
                const p = paths[key];
                return (
                  <g key={key}>
                    <path
                      data-edge={key} data-kind={e.kind} data-label={e.label ?? e.why}
                      className={`rc-edge ${e.kind}`} d={p?.d ?? "M0,0"} markerEnd={`url(#rc-ar-${e.kind})`}
                    />
                    {p && e.label && !vertical && (
                      <text x={p.lx} y={p.ly} textAnchor="middle" className={`rc-elabel ${e.kind}`}>{t(EDGE_KEY[e.label])}</text>
                    )}
                  </g>
                );
              })}
            </svg>
          </div>
        </div>
      )}
    </div>
  );
}
