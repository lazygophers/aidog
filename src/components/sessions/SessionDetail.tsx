// 会话行内展开详情（票 03）：token 消耗图（每次调用一点，堆叠四类）。
// 模型链（相邻同模型合并）与子代理区（票 04）各是折叠块（票 12）：默认收起不请求，点开才按会话读取，
// 结果缓存到本行收起（组件卸载）为止。子代理区只在会话有子代理调用时出现。
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown } from "lucide-react";
import { StackedAreaChart } from "../charts";
import { clientUsageApi, type ClientUsageAgentGroup, type ClientUsageSessionDetail } from "../../services/api/clientUsage";
import { formatNumber } from "../../utils/formatters";
import { SessionCost } from "./SessionCost";

type Load = { state: "loading" } | { state: "error"; error: string } | { state: "ready"; detail: ClientUsageSessionDetail };
type Part = "chain" | "agents";

/** 折叠块：首次展开才调 fetch，之后收起 / 展开只切显示。 */
function LazyBlock({ title, testId, fetch, children }: {
  title: string;
  testId: string;
  fetch: () => Promise<ClientUsageSessionDetail>;
  children: (d: ClientUsageSessionDetail) => ReactNode;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [load, setLoad] = useState<Load | null>(null);
  const toggle = () => {
    setOpen(!open);
    // 只在展开时读；已读到（或在读）不重复请求，失败的下次展开重试。
    if (open || (load && load.state !== "error")) return;
    setLoad({ state: "loading" });
    fetch().then(
      (detail) => setLoad({ state: "ready", detail }),
      (e) => setLoad({ state: "error", error: String(e) }),
    );
  };
  const state = !open ? "closed" : (load?.state ?? "loading");
  return (
    <div data-testid={`${testId}-block`} data-state={state}>
      <button
        type="button"
        className="mb-2 inline-flex items-center gap-1 text-sm font-medium text-muted-foreground hover:text-foreground"
        aria-expanded={open}
        data-testid={`${testId}-toggle`}
        onClick={toggle}
      >
        <ChevronDown aria-hidden className={`size-4 transition-transform motion-reduce:transition-none ${open ? "" : "-rotate-90 rtl:rotate-90"}`} />
        {title}
      </button>
      {open && load?.state === "loading" && <p className="text-sm text-muted-foreground" aria-busy="true">{t("sessions.detailLoading", "正在加载详情…")}</p>}
      {open && load?.state === "error" && (
        <p role="alert" className="break-all text-sm text-destructive">{t("sessions.detailError", "详情加载失败：{{error}}", { error: load.error })}</p>
      )}
      {open && load?.state === "ready" && children(load.detail)}
    </div>
  );
}

export function SessionDetail({ clientCode, sessionId, sidechainCalls = 0 }: { clientCode: string; sessionId: string; sidechainCalls?: number }) {
  const { t } = useTranslation();
  const [load, setLoad] = useState<Load>({ state: "loading" });

  useEffect(() => {
    let alive = true;
    setLoad({ state: "loading" });
    clientUsageApi.sessionDetail(clientCode, sessionId, "points").then(
      (detail) => { if (alive) setLoad({ state: "ready", detail }); },
      (e) => { if (alive) setLoad({ state: "error", error: String(e) }); },
    );
    return () => { alive = false; };
  }, [clientCode, sessionId]);
  const fetchPart = (part: Part) => () => clientUsageApi.sessionDetail(clientCode, sessionId, part);

  // 键序即堆叠序（自下而上）：缓存读通常最大，沉底。色板默认第 1、2 色同为紫系分不开，显式取四个可区分色。
  const config = useMemo(() => ({
    cacheRead: { label: t("sessions.seriesCacheRead", "缓存读"), color: "var(--chart-3)" },
    cacheWrite: { label: t("sessions.seriesCacheWrite", "缓存写"), color: "var(--muted-foreground)" },
    input: { label: t("sessions.seriesInput", "输入"), color: "var(--data-primary)" },
    output: { label: t("sessions.seriesOutput", "输出"), color: "var(--chart-4)" },
  }), [t]);
  const points = load.state === "ready" ? load.detail.points : [];
  const data = useMemo(() => {
    const rows = points.map((p) => ({
      x: p.callTimeMs, input: p.inputTokens, output: p.outputTokens, cacheRead: p.cacheReadTokens, cacheWrite: p.cacheWriteTokens,
    }));
    // 单次调用时 x 轴域零宽、画不出面积：前后各补一个零点（±1 分钟），画成一个尖峰。
    if (rows.length !== 1) return rows;
    const zero = (x: number) => ({ x, input: 0, output: 0, cacheRead: 0, cacheWrite: 0 });
    return [zero(rows[0].x - 60_000), rows[0], zero(rows[0].x + 60_000)];
  }, [points]);
  const agentName = (g: ClientUsageAgentGroup) =>
    g.kind === "main" ? t("sessions.agentMain", "主线") : g.agentType ?? (g.agentId ? g.agentId.slice(0, 8) : t("sessions.agentSub", "子代理"));

  const state = load.state === "ready" && load.detail.points.length === 0 ? "empty" : load.state;
  return (
    <div className="mx-3 mb-3 space-y-4 rounded-lg border bg-card p-4" data-testid="session-detail" data-state={state}>
      {load.state === "loading" && <p className="text-sm text-muted-foreground" aria-busy="true">{t("sessions.detailLoading", "正在加载详情…")}</p>}
      {load.state === "error" && (
        <p role="alert" className="break-all text-sm text-destructive">{t("sessions.detailError", "详情加载失败：{{error}}", { error: load.error })}</p>
      )}
      {state === "empty" && <p className="text-sm text-muted-foreground">{t("sessions.detailEmpty", "保留期内没有这个会话的调用明细")}</p>}
      {state === "ready" && (
        <>
          {/* 图例下留白：bare 模式图例紧贴 Y 轴顶刻度，加内边距让 Recharts 量高时把绘图区下推。 */}
          <div data-testid="session-detail-chart" data-points={String(points.length)} data-rows={String(data.length)} className="[&_.recharts-legend-wrapper]:pb-3">
            <h4 className="mb-2 text-sm font-medium text-muted-foreground">{t("sessions.detailChart", "token 消耗")}</h4>
            <StackedAreaChart bare config={config} data={data} height={200} valueFormat={formatNumber} />
          </div>
          <LazyBlock title={t("sessions.detailChain", "模型链")} testId="session-detail-chain" fetch={fetchPart("chain")}>
            {(d) => (
              <ol className="flex flex-wrap items-center gap-1.5" data-testid="session-detail-chain">
                {d.modelChain.map((s, i) => (
                  <li key={`${i}:${s.model}`} className="flex items-center gap-1.5">
                    {i > 0 && <span aria-hidden className="text-muted-foreground rtl:-scale-x-100">→</span>}
                    <span
                      className="rounded-md bg-[var(--accent-subtle)] px-2 py-0.5 font-mono text-sm"
                      title={`${formatNumber(s.inputTokens + s.outputTokens)} tokens`}
                      data-testid="session-detail-seg"
                      data-model={s.model}
                      data-calls={String(s.calls)}
                    >
                      <span dir="ltr">{s.model || "?"}</span>
                      <span className="ms-1.5 font-sans text-muted-foreground">{t("sessions.segCalls", "{{n}} 次", { n: s.calls })}</span>
                    </span>
                  </li>
                ))}
              </ol>
            )}
          </LazyBlock>
          {sidechainCalls > 0 && (
            <LazyBlock title={t("sessions.detailAgents", "主线与子代理")} testId="session-detail-agents" fetch={fetchPart("agents")}>
              {(d) => (
                <ul className="divide-y rounded-md border" data-testid="session-detail-agents">
                  {d.agents.map((g, i) => (
                    <li
                      key={`${i}:${g.agentId ?? g.kind}`}
                      className="grid grid-cols-[minmax(0,1fr)_5rem_5rem_6rem] items-center gap-4 px-3 py-2 text-sm"
                      data-testid="session-detail-agent"
                      data-kind={g.kind}
                      data-agent-id={g.agentId ?? ""}
                      data-calls={String(g.calls)}
                    >
                      <span className="truncate" title={g.agentId ?? undefined} data-testid="session-detail-agent-name">
                        {g.kind === "main"
                          ? <span className="font-medium">{agentName(g)}</span>
                          : <span dir="ltr" className={g.agentType ? "" : "font-mono"}>{agentName(g)}</span>}
                      </span>
                      <span className="text-end text-muted-foreground">{t("sessions.segCalls", "{{n}} 次", { n: g.calls })}</span>
                      <span className="text-end font-mono tabular-nums" title={t("sessions.tokenSplit", "入 {{input}} / 出 {{output}}", { input: formatNumber(g.inputTokens), output: formatNumber(g.outputTokens) })}>
                        {formatNumber(g.inputTokens + g.outputTokens)}
                      </span>
                      <span className="text-end font-mono tabular-nums">
                        <SessionCost cost={g.cost} calls={g.calls} unpricedCalls={g.unpricedCalls} testId="session-detail-agent-cost" />
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </LazyBlock>
          )}
        </>
      )}
    </div>
  );
}
