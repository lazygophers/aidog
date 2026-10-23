import { useLayoutEffect, useMemo, useRef, useState } from "react";
import type { TFunction } from "i18next";
import type { RouteReason } from "../../services/api/types/generated/RouteReason";
import type { RouteTag } from "../../services/api/types/generated/RouteTag";
import type { RoutingSnapshot } from "../../services/api/types/generated/RoutingSnapshot";
import { F } from "../../domains/shared/tokens";

/** 票 02「路由决策」区：workflow 节点画布（请求→路由→平台→响应）。设计稿 dir-a-workflow画布-v2.html。 */

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
  "model-not-on-platform": "logs.routing.reason.model-not-on-platform",
  "skipped-dead-upstream": "logs.routing.reason.skipped-dead-upstream",
};
const TAG_KEY: Record<RouteTag, string> = {
  "mapping-promoted": "logs.routing.tag.mapping-promoted",
  "coding-plan": "logs.routing.tag.coding-plan",
  "demoted": "logs.routing.tag.demoted",
  "sticky": "logs.routing.tag.sticky",
};
const MODE_KEY: Record<string, string> = {
  load_balance: "logs.routing.mode.load_balance",
  failover: "logs.routing.mode.failover",
  health_aware: "logs.routing.mode.health_aware",
  sticky: "logs.routing.mode.sticky",
};

type NodeCls = "win" | "fail" | "dim";
interface NodeSpec {
  id: string;
  cls: NodeCls;
  ico: string;
  nm: string;
  sb?: string;
  pills?: string[];
}

const EDGE_WIN = "rc-edge win";
const EDGE_BAD = "rc-edge bad";
const EDGE_EVAL = "rc-edge";

/** 胜出：最后一次 2xx 尝试；失败尝试：非 2xx；选中未尝试 / 排除：灰虚线。 */
function buildNodes(
  routing: RoutingSnapshot,
  attempts: { platform_id: number; status_code: number }[] | undefined,
  platformMap: Map<number, string>,
  winId: string | null,
  t: T,
): NodeSpec[] {
  const lastFailByPlatform = new Map<number, number>();
  attempts?.forEach((a) => {
    if (!(a.status_code >= 200 && a.status_code < 300)) lastFailByPlatform.set(a.platform_id, a.status_code);
  });

  const nodes: NodeSpec[] = [
    { id: "req", cls: "dim", ico: "→", nm: t("logs.routing.request", "请求") },
    { id: "route", cls: "dim", ico: "⇄", nm: t("logs.routing.route", "路由") },
  ];
  for (const c of routing.entries) {
    const nm = platformMap.get(c.platform_id) || `#${c.platform_id}`;
    const sb = `P${c.level_priority} · ${c.priority}`;
    const id = `p-${c.platform_id}`;
    if (c.reason !== "selected") {
      nodes.push({ id, cls: "dim", ico: "·", nm, sb, pills: [t(REASON_KEY[c.reason])] });
    } else if (id === winId) {
      nodes.push({ id, cls: "win", ico: "✓", nm, sb, pills: c.tags.map((tg) => t(TAG_KEY[tg])) });
    } else if (lastFailByPlatform.has(c.platform_id)) {
      nodes.push({ id, cls: "fail", ico: "✕", nm, sb, pills: [`${t("logs.routing.attemptFailed", "尝试失败")} ${lastFailByPlatform.get(c.platform_id)}`] });
    } else {
      nodes.push({ id, cls: "dim", ico: "·", nm, sb, pills: [t("logs.routing.untried", "未尝试")] });
    }
  }
  return nodes;
}

export function RoutingCanvas({
  routing,
  attempts,
  platformMap,
  durationMs,
  statusCode,
  t,
}: {
  routing: RoutingSnapshot | null;
  attempts: { platform_id: number; status_code: number }[] | undefined;
  platformMap: Map<number, string>;
  durationMs: number;
  statusCode: number;
  t: T;
}) {
  const canvasRef = useRef<HTMLDivElement>(null);
  const nodeRefs = useRef(new Map<string, HTMLDivElement>());
  const [width, setWidth] = useState<number | null>(null);
  const [canvasH, setCanvasH] = useState(260);
  const [edges, setEdges] = useState<{ key: string; d: string; cls: string }[]>([]);
  const [tick, setTick] = useState(0);

  const winId = useMemo(() => {
    const wins = attempts?.filter((a) => a.status_code >= 200 && a.status_code < 300);
    return wins && wins.length >= 1 ? `p-${wins[wins.length - 1]!.platform_id}` : null;
  }, [attempts]);

  const nodes = useMemo(() => {
    if (!routing || routing.entries.length < 1) return null;
    const list = buildNodes(routing, attempts, platformMap, winId, t);
    list.push({
      id: "resp",
      cls: winId !== null ? "win" : "dim",
      ico: "✓",
      nm: t("logs.routing.response", "响应"),
      sb: `${durationMs}ms · ${statusCode}`,
    });
    return list;
  }, [routing, attempts, platformMap, winId, durationMs, statusCode, t]);

  const vertical = width !== null && width < 720;

  // 宽布局：4 列绝对定位（位置与节点实测尺寸解耦）；容器 <720px 转纵向流式
  const pos = useMemo(() => {
    if (!nodes || width === null || vertical) return null;
    const colX = [24, width * 0.24, width * 0.5, width * 0.78];
    const plats = nodes.slice(2, -1);
    const rowH = 84;
    const y0 = Math.max(16, (260 - Math.max(plats.length, 1) * rowH) / 2);
    const p: Record<string, { x: number; y: number }> = {};
    plats.forEach((n, i) => { p[n.id] = { x: colX[2]!, y: y0 + i * rowH }; });
    p["req"] = { x: colX[0]!, y: 100 };
    p["route"] = { x: colX[1]!, y: 100 };
    p[nodes[nodes.length - 1]!.id] = { x: colX[3]!, y: 100 };
    return p;
  }, [nodes, width, vertical]);

  // 连线：req→route→各平台；胜出平台→响应（无胜出则 route→响应）。
  // useLayoutEffect 时 DOM 已提交，offset* 即用即取；resize 经 RO 80ms debounce 驱动 tick 重算。
  useLayoutEffect(() => {
    if (!nodes) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    setWidth(canvas.clientWidth);
    const boxes: Record<string, { l: number; t: number; w: number; h: number }> = {};
    nodeRefs.current.forEach((el, id) => {
      boxes[id] = { l: el.offsetLeft, t: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight };
    });
    const hasWin = winId !== null && boxes[winId] !== undefined;
    const pairs: [string, string, string][] = [["req", "route", hasWin ? EDGE_WIN : EDGE_EVAL]];
    for (const n of nodes.slice(2, -1)) {
      pairs.push(["route", n.id, n.cls === "win" ? EDGE_WIN : n.cls === "fail" ? EDGE_BAD : EDGE_EVAL]);
    }
    const respId = nodes[nodes.length - 1]!.id;
    pairs.push([hasWin ? winId! : "route", respId, hasWin ? EDGE_WIN : EDGE_EVAL]);
    const out: { key: string; d: string; cls: string }[] = [];
    for (const [a, b, cls] of pairs) {
      const A = boxes[a!]; const B = boxes[b!];
      if (!A || !B) continue;
      const x1c = A.l + A.w / 2, y1c = A.t + A.h / 2, x2c = B.l + B.w / 2, y2c = B.t + B.h / 2;
      let d: string;
      if (Math.abs(x2c - x1c) < Math.abs(y2c - y1c)) {
        // 正上/正下：纵向贝塞尔（上卡底边中点 → 下卡顶边中点）
        const [top, bot] = A.t < B.t ? [A, B] : [B, A];
        const y1 = top.t + top.h, y2 = bot.t;
        const dy = Math.max(22, Math.abs(y2 - y1) * 0.5);
        d = `M${x1c},${y1} C${x1c},${y1 + dy} ${x2c},${y2 - dy} ${x2c},${y2}`;
      } else {
        // 左右：水平贝塞尔
        const [l, r] = A.l < B.l ? [A, B] : [B, A];
        const dx = Math.max(36, (r.l - (l.l + l.w)) * 0.5);
        const x1 = l.l + l.w, x2 = r.l;
        d = `M${x1},${y1c} C${x1 + dx},${y1c} ${x2 - dx},${y2c} ${x2},${y2c}`;
      }
      out.push({ key: `${a}->${b}`, d, cls });
    }
    setEdges(out);
    if (pos) {
      let maxBottom = 260;
      Object.values(boxes).forEach((bx) => { maxBottom = Math.max(maxBottom, bx.t + bx.h + 24); });
      setCanvasH(maxBottom);
    }
  }, [nodes, winId, pos, tick]);

  useEffectResize(canvasRef, () => setTick((x) => x + 1));

  const modeLabel = routing?.mode != null && MODE_KEY[routing.mode] !== undefined
    ? t(MODE_KEY[routing.mode]!)
    : null;

  return (
    <div className="glass-surface hover-lift" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 10 }} data-testid="routing-canvas">
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span style={{ fontSize: F.body, fontWeight: 600 }}>{t("logs.routingDecision", "路由决策")}</span>
        {modeLabel && (
          <span
            data-testid="routing-mode"
            style={{
              fontSize: 11, padding: "2px 9px", borderRadius: 99, fontWeight: 600,
              background: "var(--accent-subtle)", color: "var(--accent)",
              border: "1px solid color-mix(in srgb, var(--accent) 30%, transparent)",
            }}
          >
            {modeLabel}
          </span>
        )}
      </div>
      {!nodes ? (
        <span style={{ fontSize: F.small, color: "var(--text-tertiary)" }} data-testid="routing-empty">
          {t("logs.routing.noSnapshot", "无路由决策记录")}
        </span>
      ) : (
        <div
          ref={canvasRef}
          data-testid="routing-flow"
          className={vertical ? "rc-vertical" : undefined}
          style={{
            position: "relative",
            height: pos ? canvasH : undefined,
            display: "flex",
            flexDirection: "column",
            alignItems: pos ? undefined : "flex-start",
            gap: vertical ? 34 : undefined,
          }}
        >
          <svg style={{ position: "absolute", inset: 0, width: "100%", height: "100%", pointerEvents: "none" }}>
            {edges.map((e) => <path key={e.key} className={e.cls} d={e.d} />)}
          </svg>
          {nodes.map((n, i) => {
            const isResp = i === nodes.length - 1;
            return (
              <div
                key={n.id}
                ref={(el) => { if (el) nodeRefs.current.set(n.id, el); else nodeRefs.current.delete(n.id); }}
                data-node-id={n.id}
                data-state={isResp ? "resp" : n.cls}
                className={`rc-node ${n.cls}`}
                style={pos?.[n.id] ? { position: "absolute", left: pos[n.id]!.x, top: pos[n.id]!.y } : undefined}
              >
                <span className="rc-port in" aria-hidden="true" />
                <span className="rc-ico">{n.ico}</span>
                <span style={{ display: "flex", flexDirection: "column", minWidth: 0, alignItems: "flex-start" }}>
                  <span className="rc-nm">{n.nm}</span>
                  {n.sb && <span className="rc-sb">{n.sb}</span>}
                  {n.pills?.map((pl, j) => <span key={j} className={`rc-why ${n.cls === "fail" ? "red" : n.cls === "win" ? "green" : ""}`}>{pl}</span>)}
                </span>
                <span className="rc-port out" aria-hidden="true" />
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** ResizeObserver 容器宽度重算连线（80ms debounce；jsdom 无 RO 时跳过）。 */
function useEffectResize(ref: React.RefObject<HTMLDivElement | null>, onChange: () => void) {
  const cbRef = useRef(onChange);
  cbRef.current = onChange;
  useLayoutEffect(() => {
    if (typeof ResizeObserver === "undefined") return;
    const el = ref.current;
    if (!el) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ro = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => cbRef.current(), 80);
    });
    ro.observe(el);
    return () => { ro.disconnect(); clearTimeout(timer); };
  }, [ref]);
}
