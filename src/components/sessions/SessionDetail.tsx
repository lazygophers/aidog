// 会话行内展开详情（票 03）：模型链（相邻同模型合并）+ token 消耗图（每次调用一点，堆叠四类）。
// 子代理区由票 04 填充，这里只留挂载位。
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { StackedAreaChart } from "../charts";
import { clientUsageApi, type ClientUsageSessionDetail } from "../../services/api/clientUsage";
import { formatNumber } from "../../utils/formatters";

type Load = { state: "loading" } | { state: "error"; error: string } | { state: "ready"; detail: ClientUsageSessionDetail };

export function SessionDetail({ clientCode, sessionId }: { clientCode: string; sessionId: string }) {
  const { t } = useTranslation();
  const [load, setLoad] = useState<Load>({ state: "loading" });

  useEffect(() => {
    let alive = true;
    setLoad({ state: "loading" });
    clientUsageApi.sessionDetail(clientCode, sessionId).then(
      (detail) => { if (alive) setLoad({ state: "ready", detail }); },
      (e) => { if (alive) setLoad({ state: "error", error: String(e) }); },
    );
    return () => { alive = false; };
  }, [clientCode, sessionId]);

  // 键序即堆叠序（自下而上）：缓存读通常最大，沉底。色板默认第 1、2 色同为紫系分不开，显式取四个可区分色。
  const config = useMemo(() => ({
    cacheRead: { label: t("sessions.seriesCacheRead", "缓存读"), color: "var(--chart-3)" },
    cacheWrite: { label: t("sessions.seriesCacheWrite", "缓存写"), color: "var(--muted-foreground)" },
    input: { label: t("sessions.seriesInput", "输入"), color: "var(--data-primary)" },
    output: { label: t("sessions.seriesOutput", "输出"), color: "var(--chart-4)" },
  }), [t]);
  const points = load.state === "ready" ? load.detail.points : [];
  const data = useMemo(() => points.map((p) => ({
    x: p.callTimeMs, input: p.inputTokens, output: p.outputTokens, cacheRead: p.cacheReadTokens, cacheWrite: p.cacheWriteTokens,
  })), [points]);

  const state = load.state === "ready" && load.detail.points.length === 0 ? "empty" : load.state;
  return (
    <div className="mx-3 mb-3 space-y-4 rounded-lg border bg-card p-4" data-testid="session-detail" data-state={state}>
      {load.state === "loading" && <p className="text-sm text-muted-foreground" aria-busy="true">{t("sessions.detailLoading", "正在加载详情…")}</p>}
      {load.state === "error" && (
        <p role="alert" className="break-all text-sm text-destructive">{t("sessions.detailError", "详情加载失败：{{error}}", { error: load.error })}</p>
      )}
      {state === "empty" && <p className="text-sm text-muted-foreground">{t("sessions.detailEmpty", "保留期内没有这个会话的调用明细")}</p>}
      {state === "ready" && load.state === "ready" && (
        <>
          <div>
            <h4 className="mb-2 text-sm font-medium text-muted-foreground">{t("sessions.detailChain", "模型链")}</h4>
            <ol className="flex flex-wrap items-center gap-1.5" data-testid="session-detail-chain">
              {load.detail.modelChain.map((s, i) => (
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
          </div>
          {/* 图例下留白：bare 模式图例紧贴 Y 轴顶刻度，加内边距让 Recharts 量高时把绘图区下推。 */}
          <div data-testid="session-detail-chart" data-points={String(points.length)} className="[&_.recharts-legend-wrapper]:pb-3">
            <h4 className="mb-2 text-sm font-medium text-muted-foreground">{t("sessions.detailChart", "token 消耗")}</h4>
            <StackedAreaChart bare config={config} data={data} height={200} valueFormat={formatNumber} />
          </div>
        </>
      )}
      <div data-testid="session-detail-subagents-slot" />
    </div>
  );
}
