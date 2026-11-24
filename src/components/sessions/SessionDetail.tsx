// 会话行内展开详情（票 03）：token 消耗图（每次调用一点，堆叠四类）。
// 模型链（相邻同模型合并）与子代理区（票 04）各是折叠块（票 12）：默认收起不请求，点开才按会话读取，
// 结果缓存到本行收起（组件卸载）为止。子代理区只在会话有子代理调用时出现。
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown } from "lucide-react";
import { LineChart, StackedAreaChart } from "../charts";
import { clientUsageApi, type ClientUsageAgentGroup, type ClientUsageSessionDetail } from "../../services/api/clientUsage";
import { formatNumber } from "../../utils/formatters";
import { SessionCost } from "./SessionCost";

type Load<T> = { state: "loading" } | { state: "error"; error: string } | { state: "ready"; detail: T };
type Part = "chain" | "agents" | "interactions";

/** 折叠块：首次展开才调 fetch，之后收起 / 展开只切显示。 */
function LazyBlock<T = never>({ title, testId, fetch, content, children }: {
  title: string;
  testId: string;
  fetch?: () => Promise<T>;
  content?: () => ReactNode;
  children?: (d: T) => ReactNode;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [load, setLoad] = useState<Load<T> | null>(null);
  const toggle = () => {
    setOpen(!open);
    // 只在展开时读；已读到（或在读）不重复请求，失败的下次展开重试。
    if (!fetch || open || (load && load.state !== "error")) return;
    setLoad({ state: "loading" });
    fetch().then(
      (detail) => setLoad({ state: "ready", detail }),
      (e) => setLoad({ state: "error", error: String(e) }),
    );
  };
  const state = !open ? "closed" : (fetch ? (load?.state ?? "loading") : "ready");
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
      {open && load?.state === "loading" && <p className="text-sm text-muted-foreground" aria-busy="true">{t("sessions.detailLoading", "Loading details…")}</p>}
      {open && load?.state === "error" && (
        <p role="alert" className="break-all text-sm text-destructive">{t("sessions.detailError", "Failed to load details: {{error}}", { error: load.error })}</p>
      )}
      {open && (fetch
        ? load?.state === "ready" && children?.(load.detail)
        : content?.())}
    </div>
  );
}

export function SessionDetail({ clientCode, sessionId, sidechainCalls = 0 }: { clientCode: string; sessionId: string; sidechainCalls?: number }) {
  const { t } = useTranslation();
  const [load, setLoad] = useState<Load<ClientUsageSessionDetail>>({ state: "loading" });

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
    cacheRead: { label: t("sessions.seriesCacheRead", "Cache read"), color: "var(--chart-3)" },
    cacheWrite: { label: t("sessions.seriesCacheWrite", "Cache write"), color: "var(--muted-foreground)" },
    input: { label: t("sessions.seriesInput", "Input"), color: "var(--data-primary)" },
    output: { label: t("sessions.seriesOutput", "Output"), color: "var(--chart-4)" },
  }), [t]);
  const points = load.state === "ready" ? load.detail.points : [];
  const contextConfig = useMemo(() => ({
    context: { label: t("sessions.contextTokens", "Prompt tokens per call"), color: "var(--data-primary)" },
  }), [t]);
  const contextData = useMemo(() => points.map((p) => ({ x: p.callTimeMs, context: p.inputTokens })), [points]);
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
    g.kind === "main" ? t("sessions.agentMain", "Main thread") : g.agentType ?? (g.agentId ? g.agentId.slice(0, 8) : t("sessions.agentSub", "Subagent"));

  const state = load.state === "ready" && load.detail.points.length === 0 ? "empty" : load.state;
  return (
    <div className="mx-3 mb-3 space-y-4 rounded-lg border bg-card p-4" data-testid="session-detail" data-state={state}>
      {load.state === "loading" && <p className="text-sm text-muted-foreground" aria-busy="true">{t("sessions.detailLoading", "Loading details…")}</p>}
      {load.state === "error" && (
        <p role="alert" className="break-all text-sm text-destructive">{t("sessions.detailError", "Failed to load details: {{error}}", { error: load.error })}</p>
      )}
      {state === "empty" && <p className="text-sm text-muted-foreground">{t("sessions.detailEmpty", "No call records for this session within the retention period")}</p>}
      {load.state === "ready" && (
        <>
          {/* 图例下留白：bare 模式图例紧贴 Y 轴顶刻度，加内边距让 Recharts 量高时把绘图区下推。 */}
          <div data-testid="session-detail-chart" data-points={String(points.length)} data-rows={String(data.length)} className="[&_.recharts-legend-wrapper]:pb-3">
            <h4 className="mb-2 text-sm font-medium text-muted-foreground">{t("sessions.detailChart", "Token usage")}</h4>
            <StackedAreaChart bare config={config} data={data} height={200} valueFormat={formatNumber} />
          </div>
          <div data-testid="session-detail-context-chart" data-points={String(contextData.length)}>
            <h4 className="mb-2 text-sm font-medium text-muted-foreground">{t("sessions.contextChart", "Context tokens per call")}</h4>
            <LineChart config={contextConfig} data={contextData} height={180} valueFormat={formatNumber} />
          </div>
          <LazyBlock title={t("sessions.detailHistory", "Interactions")} testId="session-detail-history" fetch={fetchPart("interactions")}>
            {(d) => d.interactions.length === 0
              ? <p className="text-sm text-muted-foreground">{t("sessions.detailHistoryEmpty", "No interactions found in the source files.")}</p>
              : <ol className="max-h-[32rem] list-none space-y-3 overflow-y-auto rounded-md border p-3" data-testid="session-detail-history-list">
                {d.interactions.map((item, i) => (
                  <li key={`${item.timestamp}:${i}`} className="min-w-0 border-b pb-3 last:border-0 last:pb-0" data-testid="session-detail-interaction" data-role={item.role}>
                    <div className="mb-1 flex flex-wrap justify-between gap-2 text-sm text-muted-foreground">
                      <span dir="ltr">{item.role}</span><time dir="ltr">{item.timestamp}</time>
                    </div>
                    <pre dir="auto" className="whitespace-pre-wrap break-words font-sans text-sm">{item.content}</pre>
                  </li>
                ))}
              </ol>}
          </LazyBlock>
          <LazyBlock title={t("sessions.detailChain", "Model chain")} testId="session-detail-chain" fetch={fetchPart("chain")}>
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
                      <span className="ms-1.5 font-sans text-muted-foreground">{t("sessions.segCalls", "{{n}} calls", { n: s.calls })}</span>
                    </span>
                  </li>
                ))}
              </ol>
            )}
          </LazyBlock>
          {sidechainCalls > 0 && (
            <LazyBlock title={t("sessions.detailAgents", "Main thread & subagents")} testId="session-detail-agents" fetch={fetchPart("agents")}>
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
                      <span className="text-end text-muted-foreground">{t("sessions.segCalls", "{{n}} calls", { n: g.calls })}</span>
                      <span className="text-end font-mono tabular-nums" title={t("sessions.tokenSplit", "in {{input}} / out {{output}}", { input: formatNumber(g.inputTokens), output: formatNumber(g.outputTokens) })}>
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
