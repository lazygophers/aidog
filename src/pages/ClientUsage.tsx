// ClientUsage.tsx — 日志统计 → 客户端统计（issue #65 票 02b）。
// 读取开关在 设置 › 客户端会话读取（扩展-会话页 票 01）；四家都未启用只见引导卡，部分启用顶部一行提示。

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import { Input } from "../components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
import { LineChart } from "../components/charts";
import type { ChartConfig } from "../components/ui/chart";
import {
  clientUsageApi,
  type ClientUsageModelRow,
  type ClientUsageSessionRow,
  type ClientUsageStatus,
  type ClientUsageSummary,
  type ClientUsageTrendRow,
} from "../services/api/clientUsage";
import { formatNumber, formatMoney } from "../utils/formatters";

// 客户端大类码（票 02 支持的读取器；显示名为品牌专名不译）。
const CLIENTS = [
  { code: "claude_code", name: "Claude Code" },
  { code: "codex", name: "Codex" },
  { code: "gemini_cli", name: "Gemini CLI" },
  { code: "opencode", name: "OpenCode" },
] as const;

type Preset = "today" | "7d" | "30d" | "custom";

function presetRange(preset: Preset, from: string, to: string): { fromMs: number; toMs: number } {
  const now = new Date();
  if (preset === "custom" && from && to) {
    const s = new Date(`${from}T00:00:00`);
    const e = new Date(`${to}T23:59:59.999`);
    if (!Number.isNaN(s.getTime()) && !Number.isNaN(e.getTime())) {
      return { fromMs: s.getTime(), toMs: e.getTime() };
    }
  }
  if (preset === "today") {
    const s = new Date(now);
    s.setHours(0, 0, 0, 0);
    return { fromMs: s.getTime(), toMs: now.getTime() };
  }
  const days = preset === "7d" ? 7 : 30;
  const s = new Date(now);
  s.setDate(s.getDate() - days);
  return { fromMs: s.getTime(), toMs: now.getTime() };
}

// 模型花费列近似口径：cost<=0 视为未定价（免费模型 cost=0 会误标；会话行已改用后端 unpricedCalls 真实字段）。
const isUnpriced = (cost: number) => !(cost > 0);

export function ClientUsage({ onNavigate }: { onNavigate?: (id: string) => void } = {}) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ClientUsageStatus | null>(null);
  const [error, setError] = useState("");

  const [preset, setPreset] = useState<Preset>("7d");
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");

  const [summary, setSummary] = useState<ClientUsageSummary | null>(null);
  const [trend, setTrend] = useState<ClientUsageTrendRow[]>([]);
  const [byModel, setByModel] = useState<ClientUsageModelRow[]>([]);
  const [sessions, setSessions] = useState<ClientUsageSessionRow[]>([]);
  const [dataError, setDataError] = useState("");
  const [loaded, setLoaded] = useState(false);

  const enabled = status?.consent.enabled ?? {};
  const anyEnabled = CLIENTS.some((c) => enabled[c.code]);
  const disabledClients = CLIENTS.filter((c) => !enabled[c.code]);
  const goSettings = () => onNavigate?.("settings/client_usage");
  const importing = status?.import.running ?? false;

  const range = useMemo(
    () => presetRange(preset, fromDate, toDate),
    [preset, fromDate, toDate],
  );

  const loadStatus = useCallback(async () => {
    try {
      const s = await clientUsageApi.status();
      setStatus(s);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const loadData = useCallback(async (r: { fromMs: number; toMs: number }) => {
    setLoaded(false);
    setDataError("");
    const [sum, tr, models, sess] = await Promise.all([
      clientUsageApi.summary(r.fromMs, r.toMs).catch((e) => { setDataError(String(e)); return null; }),
      clientUsageApi.trend(r.fromMs, r.toMs).catch(() => [] as ClientUsageTrendRow[]),
      clientUsageApi.byModel(r.fromMs, r.toMs).catch(() => [] as ClientUsageModelRow[]),
      clientUsageApi.sessions(r.fromMs, r.toMs).catch(() => [] as ClientUsageSessionRow[]),
    ]);
    if (sum) setSummary(sum);
    setTrend(tr);
    setByModel(models);
    setSessions(sess);
    setLoaded(true);
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  useEffect(() => {
    if (anyEnabled) void loadData(range);
  }, [anyEnabled, range, loadData]);

  // 导入进行中轮询 status；结束后刷一次数据。
  useEffect(() => {
    if (!importing) return;
    const timer = setTimeout(() => { void loadStatus(); }, 1500);
    return () => clearTimeout(timer);
  }, [importing, status, loadStatus]);

  const refresh = async () => {
    setError("");
    try {
      const st = await clientUsageApi.refresh();
      setStatus((s) => (s ? { ...s, import: st } : s));
    } catch (e) {
      setError(String(e));
    }
  };

  // 趋势图数据：calls 与 cost 量纲差一个数量级以上，拆两张图各一条线。
  const trendCalls = useMemo(
    () => trend.map((r) => ({ x: r.dayMs, calls: r.calls })),
    [trend],
  );
  const trendCost = useMemo(
    () => trend.map((r) => ({ x: r.dayMs, cost: r.cost })),
    [trend],
  );
  const callsConfig: ChartConfig = { calls: { label: t("clientUsage.calls", "调用数") } };
  const costConfig: ChartConfig = { cost: { label: t("clientUsage.cost", "折算花费") } };

  const imp = status?.import;
  const empty = !summary || summary.calls === 0;

  if (status && !anyEnabled) {
    // 四家都未启用：引导卡跳设置子页。
    return (
      <section className="space-y-5" data-testid="client-usage-page">
        <header>
          <h1 className="text-xl font-semibold">{t("clientUsage.title", "客户端统计")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("clientUsage.desc", "读取本地 AI 编码客户端的会话文件，按官方 API 价格折算本地用量。数据仅本机处理与存储。")}</p>
        </header>
        <Card className="p-5 space-y-4" data-testid="client-usage-guide">
          <h2 className="font-medium">{t("clientUsage.guideTitle", "尚未启用客户端读取")}</h2>
          <p className="text-sm text-muted-foreground">{t("clientUsage.guideDesc", "在设置中开启要统计的客户端。AiDog 只记录 token、成本等元数据，对话正文不入库。")}</p>
          <Button data-testid="client-usage-guide-go" onClick={goSettings}>
            {t("clientUsage.guideGo", "去设置启用")}
          </Button>
        </Card>
        {error && <p role="alert" className="text-sm text-destructive" data-testid="page-error">{error}</p>}
      </section>
    );
  }

  return (
    <section className="space-y-5" data-testid="client-usage-page">
      <header className="flex items-start justify-between gap-4 flex-wrap">
        <div>
          <h1 className="text-xl font-semibold">{t("clientUsage.title", "客户端统计")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("clientUsage.desc", "读取本地 AI 编码客户端的会话文件，按官方 API 价格折算本地用量。数据仅本机处理与存储。")}</p>
        </div>
        <Button variant="outline" data-testid="client-usage-refresh" disabled={importing} onClick={() => void refresh()}>
          {importing ? t("clientUsage.refreshing", "正在导入…") : t("clientUsage.refresh", "立即刷新")}
        </Button>
      </header>

      {error && <p role="alert" className="text-sm text-destructive" data-testid="page-error">{error}</p>}
      {status && disabledClients.length > 0 && (
        <p className="text-sm text-muted-foreground" data-testid="client-usage-partial-hint" data-disabled={disabledClients.map((c) => c.code).join(",")}>
          {t("clientUsage.partialHint", "未启用读取：{{names}}。", { names: disabledClients.map((c) => c.name).join(", ") })}{" "}
          <button type="button" className="text-primary underline-offset-2 hover:underline" data-testid="client-usage-partial-go" onClick={goSettings}>
            {t("clientUsage.guideGo", "去设置启用")}
          </button>
        </p>
      )}

      {/* 导入状态（开关已迁到设置子页）。 */}
      {(importing || (imp && imp.callsImported > 0) || (imp && !!imp.error) || (status?.persistedParseErrors ?? 0) > 0) && (
          <Card className="p-4 text-sm text-muted-foreground space-y-1" data-testid="import-state">
            {importing && (
              <p data-testid="import-progress">
                {t("clientUsage.importing", "导入中：{{done}}/{{total}} 个文件", { done: imp?.filesDone ?? 0, total: imp?.filesTotal ?? 0 })}
              </p>
            )}
            {!importing && imp && imp.callsImported > 0 && (
              <p data-testid="import-done">
                {t("clientUsage.importDone", "上次导入新增 {{n}} 条调用", { n: imp.callsImported })}
              </p>
            )}
            {!!imp?.error && <p className="text-destructive" data-testid="import-error">{imp.error}</p>}
            {(status?.persistedParseErrors ?? 0) > 0 && (
              <p data-testid="import-persisted-errors">
                {t("clientUsage.parseErrors", "累计解析错误 {{n}} 条", { n: status?.persistedParseErrors })}
              </p>
            )}
          </Card>
        )}

      {/* 时间范围 + 汇总卡。 */}
      <div className="flex items-center gap-3 flex-wrap" data-testid="range-picker">
        <Select value={preset} onValueChange={(v) => setPreset(v as Preset)}>
          <SelectTrigger className="w-40" data-testid="range-select"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="today">{t("stats.today", "今天")}</SelectItem>
            <SelectItem value="7d">{t("stats.7d", "近 7 天")}</SelectItem>
            <SelectItem value="30d">{t("stats.30d", "近 30 天")}</SelectItem>
            <SelectItem value="custom">{t("clientUsage.rangeCustom", "自定义")}</SelectItem>
          </SelectContent>
        </Select>
        {preset === "custom" && (
          <>
            <Input type="date" className="w-40" value={fromDate} onChange={(e) => setFromDate(e.target.value)} aria-label={t("clientUsage.from", "开始日期")} data-testid="range-from" />
            <Input type="date" className="w-40" value={toDate} onChange={(e) => setToDate(e.target.value)} aria-label={t("clientUsage.to", "结束日期")} data-testid="range-to" />
          </>
        )}
      </div>

      {dataError && <p role="alert" className="text-sm text-destructive" data-testid="data-error">{dataError}</p>}

      <Card className="p-4" data-testid="summary-card">
        {loaded && <span data-testid="data-loaded" className="hidden" aria-hidden="true" />}
        {empty ? (
          <p className="text-sm text-muted-foreground" data-testid="summary-empty">
            {t("clientUsage.empty", "所选时间范围内暂无数据")}
          </p>
        ) : (
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
            {([
              ["calls", t("clientUsage.calls", "调用数"), formatNumber(summary.calls)],
              ["input", t("clientUsage.input", "输入"), formatNumber(summary.inputTokens)],
              ["output", t("clientUsage.output", "输出"), formatNumber(summary.outputTokens)],
              ["cacheRead", t("clientUsage.cacheRead", "缓存读"), formatNumber(summary.cacheReadTokens)],
              ["cacheWrite", t("clientUsage.cacheWrite", "缓存写"), formatNumber(summary.cacheWriteTokens)],
              ["reasoning", t("clientUsage.reasoning", "推理"), formatNumber(summary.reasoningTokens)],
              ["cost", t("clientUsage.cost", "折算花费"), formatMoney(summary.cost)],
              ["unpriced", t("clientUsage.unpricedCalls", "未定价调用"), formatNumber(summary.unpricedCalls)],
            ] as const).map(([key, label, value]) => (
              <div key={key} data-testid={`summary-${key}`}>
                <p className="text-sm text-muted-foreground">{label}</p>
                <p className="mt-1 text-lg font-semibold tabular-nums">{value}</p>
              </div>
            ))}
          </div>
        )}
      </Card>

      {!empty && (
        <>
          <div className="grid gap-5 lg:grid-cols-2">
            <div data-testid="trend-calls">
              <LineChart
                config={callsConfig}
                data={trendCalls}
                xKey="x"
                title={t("clientUsage.trendCallsTitle", "调用趋势")}
                emptyHint={t("clientUsage.empty", "所选时间范围内暂无数据")}
              />
            </div>
            <div data-testid="trend-cost">
              <LineChart
                config={costConfig}
                data={trendCost}
                xKey="x"
                title={t("clientUsage.trendCostTitle", "折算花费趋势")}
                valueFormat={(n) => formatMoney(n)}
                emptyHint={t("clientUsage.empty", "所选时间范围内暂无数据")}
              />
            </div>
          </div>

          <Card className="p-4" data-testid="by-model-card">
            <h2 className="font-medium mb-3">{t("clientUsage.byModelTitle", "按模型分布")}</h2>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("clientUsage.model", "模型")}</TableHead>
                  <TableHead className="text-right">{t("clientUsage.calls", "调用数")}</TableHead>
                  <TableHead className="text-right">{t("clientUsage.input", "输入")}</TableHead>
                  <TableHead className="text-right">{t("clientUsage.output", "输出")}</TableHead>
                  <TableHead className="text-right">{t("clientUsage.cost", "折算花费")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {byModel.map((r) => (
                  <TableRow key={r.model} data-testid="model-row">
                    <TableCell className="font-mono text-sm">{r.model || "-"}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatNumber(r.calls)}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatNumber(r.inputTokens)}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatNumber(r.outputTokens)}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {isUnpriced(r.cost)
                        ? <span data-testid="unpriced-mark">{t("clientUsage.unpriced", "未定价")}{r.unpricedCalls > 0 ? ` ×${r.unpricedCalls}` : ""}</span>
                        : formatMoney(r.cost)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Card>

          <Card className="p-4" data-testid="sessions-card">
            <h2 className="font-medium mb-1">{t("clientUsage.sessionTitle", "会话列表")}</h2>
            <p className="text-sm text-muted-foreground mb-3">{t("clientUsage.sessionsLimit", "仅显示最近 200 个会话")}</p>
            {sessions.length === 0 ? (
              <p className="text-sm text-muted-foreground" data-testid="sessions-empty">{t("clientUsage.sessionEmpty", "所选时间范围内暂无会话")}</p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("clientUsage.client", "客户端")}</TableHead>
                    <TableHead>{t("clientUsage.session", "会话")}</TableHead>
                    <TableHead className="text-right">{t("clientUsage.calls", "调用数")}</TableHead>
                    <TableHead className="text-right">{t("clientUsage.tokens", "Token")}</TableHead>
                    <TableHead className="text-right">{t("clientUsage.cost", "折算花费")}</TableHead>
                    <TableHead className="text-right">{t("clientUsage.lastCall", "最后调用")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {sessions.map((s) => (
                    <TableRow key={`${s.clientCode}:${s.sessionId}`} data-testid="session-row">
                      <TableCell className="text-sm">{CLIENTS.find((c) => c.code === s.clientCode)?.name ?? s.clientCode}</TableCell>
                      <TableCell className="max-w-72">
                        <p className="truncate text-sm" title={s.title}>{s.title || s.sessionId}</p>
                        {s.workdir && <p className="truncate text-sm text-muted-foreground" title={s.workdir}>{s.workdir}</p>}
                      </TableCell>
                      <TableCell className="text-right tabular-nums">{formatNumber(s.calls)}</TableCell>
                      <TableCell className="text-right tabular-nums">{formatNumber(s.inputTokens + s.outputTokens)}</TableCell>
                      <TableCell className="text-right tabular-nums">
                        {s.unpricedCalls === 0
                          ? formatMoney(s.cost)
                          : s.unpricedCalls === s.calls
                            ? <span data-testid="session-unpriced">{t("clientUsage.unpriced", "未定价")}</span>
                            : <span data-testid="session-mixed-cost">{formatMoney(s.cost)} <span className="text-sm text-muted-foreground">{t("clientUsage.unpriced", "未定价")} ×{s.unpricedCalls}</span></span>}
                      </TableCell>
                      <TableCell className="text-right text-sm tabular-nums">
                        {new Date(s.lastCallMs).toLocaleString()}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </Card>
        </>
      )}
    </section>
  );
}
