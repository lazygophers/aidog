// ─── 扩展 → 上下文压缩（RTK / sqz）────────────────────────────
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Bar, CartesianGrid, ComposedChart, Line, XAxis, YAxis } from "recharts";
import { Button } from "../components/ui/button";
import { Switch } from "../components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../components/ui/tabs";
import { ChartContainer, type ChartConfig } from "../components/ui/chart";
import { ChartCard, ChartsTooltip, PRIMARY_COLOR, niceTicks, useDrawIn } from "../components/charts";
import { contextCompressApi, type ContextCompressStats, type ContextCompressStatus, type RtkSetResult } from "../services/api/contextCompress";
import { formatInteger, formatNumber, formatPercent } from "../utils/formatters";

type Range = "30" | "90" | "all";

// 每日图悬浮提示：日期 + 已使用 / 已压缩 / 压缩比例；sqz CLI 无每日输入与比率（used 缺省）时只显示已压缩。
export function DailyTooltip({ active, label, payload }: {
  active?: boolean;
  label?: string;
  payload?: { payload?: { used?: number; compressed?: number; pct?: number } }[];
}) {
  const { t } = useTranslation();
  if (!active || !payload?.length) return null;
  const row = payload[0]?.payload ?? {};
  const tipRow = (key: string, labelKey: string, value: string) => <div
    className="flex w-full items-center justify-between gap-3"
    data-testid={`cc-tip-${key}`}
  >
    <span className="text-muted-foreground">{t(labelKey)}</span>
    <span className="font-mono font-medium tabular-nums">{value}</span>
  </div>;
  return <div
    dir="auto"
    className="pointer-events-none grid min-w-[8rem] items-start gap-1.5 rounded-lg border border-border/50 bg-background px-2.5 py-1.5 text-sm shadow-xl"
    data-testid="cc-tip"
  >
    <div className="font-medium">{label}</div>
    {row.used != null && tipRow("used", "contextCompress.statsColUsed", formatInteger(row.used))}
    {tipRow("compressed", "contextCompress.statsColCompressed", formatInteger(row.compressed ?? 0))}
    {row.used != null && tipRow("ratio", "contextCompress.statsColRatio", row.used > 0 && row.pct != null ? formatPercent(row.pct) : "—")}
  </div>;
}

const COLORS = { used: "var(--chart-3)", compressed: PRIMARY_COLOR, pct: "var(--chart-4)" };
const SERIES = { used: "contextCompress.statsColUsed", compressed: "contextCompress.statsColCompressed", pct: "contextCompress.statsColRatio" };

const isoDay = (d: Date) => {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
};

export function ContextCompress() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ContextCompressStatus | null>(null);
  const [stats, setStats] = useState<ContextCompressStats | null>(null);
  const [statsState, setStatsState] = useState<"loading" | "ok" | "error">("loading");
  const [statsError, setStatsError] = useState("");
  const [error, setError] = useState("");
  const [sideErrors, setSideErrors] = useState<{ claude?: string | null; codex?: string | null }>({});
  const [busy, setBusy] = useState(false);
  const [range, setRange] = useState<Range>("30");
  const drawIn = useDrawIn();

  // stats 跑 rtk gain / sqz stats 要数秒，与 status 分开加载，不拖住开关区。
  const loadStats = useCallback(async () => {
    setStatsState("loading");
    try {
      setStats(await contextCompressApi.stats());
      setStatsState("ok");
    } catch (e) {
      setStatsError(String(e));
      setStatsState("error");
    }
  }, []);

  useEffect(() => {
    contextCompressApi.status().then(setStatus, e => setError(String(e)));
    void loadStats();
  }, [loadStats]);

  const run = async (action: () => Promise<ContextCompressStatus | RtkSetResult>) => {
    setBusy(true);
    setError("");
    try {
      const res = await action();
      if ("status" in res) {
        setStatus(res.status);
        setSideErrors({ claude: res.claude_error, codex: res.codex_error });
      } else {
        setStatus(res);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const install = (tool: "rtk" | "sqz") => run(() => contextCompressApi.install(tool));
  const label = (version: string | null) => !status
    ? t("contextCompress.checking")
    : version
    ? t("contextCompress.installed", { version })
    : t("contextCompress.notInstalled");

  const rangeHeader = <div className="flex items-center justify-between gap-4">
    <h2 className="text-base font-semibold">{t("contextCompress.statsTitle")}</h2>
    <div className="flex items-center gap-2">
      <div role="radiogroup" aria-label={t("contextCompress.statsRange")} className="flex items-center rounded-md border p-0.5 text-sm">
        {(["30", "90", "all"] as const).map(r => <button key={r} role="radio" aria-checked={range === r} type="button"
          className={`rounded px-2 py-1 ${range === r ? "bg-secondary text-secondary-foreground" : "text-muted-foreground"}`}
          data-testid={`cc-range-${r}`} onClick={() => setRange(r)}>
          {t(`contextCompress.statsRange${r === "all" ? "All" : r}`)}
        </button>)}
      </div>
      <Button variant="outline" size="sm" disabled={statsState === "loading"} onClick={() => void loadStats()} data-testid="cc-stats-refresh">{t("contextCompress.statsRefresh")}</Button>
    </div>
  </div>;

  const card = (tool: "rtk" | "sqz", tiles: [string, string, string, "range" | "total"][] | null, chart: ReactNode, extra?: ReactNode) => {
    const state = statsState !== "ok" ? statsState
      : tiles ? "ok"
      : status && !version(tool) ? "not-installed"
      : "empty";
    return <div className="space-y-3 rounded-lg border p-4" data-testid={`cc-stats-${tool}`} data-state={state}>
      {rangeHeader}
      {state === "ok" && tiles
        ? <>
          <dl className="grid grid-cols-2 gap-3 sm:grid-cols-3">
            {tiles.map(([key, labelKey, value, scope]) => <div key={key}>
              <dt className="text-sm text-muted-foreground">{t(labelKey)}{scope === "total" && <span className="ml-1 text-sm">({t("contextCompress.statsTotalScope")})</span>}</dt>
              <dd className="text-lg font-semibold tabular-nums" data-testid={`cc-stat-${tool}-${key}`}>{value}</dd>
            </div>)}
          </dl>
          {extra}
          {chart}
        </>
        : <p className={`text-sm ${state === "error" ? "text-destructive" : "text-muted-foreground"}`} data-testid={`cc-stats-${tool}-msg`}>
          {state === "loading" ? t("contextCompress.statsLoading")
            : state === "error" ? `${t("contextCompress.statsError")} ${statsError}`
            : state === "not-installed" ? t("contextCompress.statsNotInstalled")
            : t("contextCompress.statsEmpty")}
        </p>}
    </div>;
  };
  const version = (tool: "rtk" | "sqz") => tool === "rtk" ? status?.rtk_version : status?.sqz_version;

  // 区间过滤：30/90 天按本地日期算；"all" 不过滤。sqz CLI 无每日比率 / in-out，
  // 其 rate / count / in / out 只能是累计口径，UI 显式标注。
  const inRange = <T extends { date: string }>(rows: T[] | undefined): T[] => {
    if (!rows) return [];
    if (range === "all") return rows;
    const d = new Date();
    d.setDate(d.getDate() - (Number(range) - 1));
    const cutoff = isoDay(d);
    return rows.filter(r => r.date >= cutoff);
  };
  const rtkRange = useMemo(() => inRange(stats?.rtk_daily), [stats, range]);
  const sqzRange = useMemo(() => inRange(stats?.sqz_daily), [stats, range]);
  const sum = (rows: { saved_tokens: number }[]) => rows.reduce((a, r) => a + r.saved_tokens, 0);
  // 每日图：已使用 = 压缩前 tokens（rtk input_tokens），已压缩 = 节省 tokens，比例 = 当日 savings_pct（右轴）。
  // sqz CLI 无每日输入 / 比率，只画已压缩柱。横轴按日期升序。
  const chartConfig = useMemo<ChartConfig>(() => Object.fromEntries(
    (Object.keys(COLORS) as (keyof typeof COLORS)[]).map(k => [k, { label: t(SERIES[k]), color: COLORS[k] }])), [t]);
  type DayPoint = { date: string; used?: number; compressed: number; pct?: number };
  const dailyChart = (tool: "rtk" | "sqz", rows: DayPoint[]) => {
    const data = [...rows].sort((a, b) => a.date.localeCompare(b.date));
    const withRatio = tool === "rtk";
    const keys: (keyof typeof COLORS)[] = withRatio ? ["used", "compressed", "pct"] : ["compressed"];
    const yTicks = niceTicks(0, Math.max(0, ...data.map(r => Math.max(r.used ?? 0, r.compressed))), 5);
    return <div className="mt-3 space-y-2" data-testid={`cc-chart-${tool}`}>
      <h3 className="text-sm font-medium">{t("contextCompress.statsDaily")}</h3>
      <div className="flex flex-wrap items-center gap-4 text-sm text-muted-foreground" data-testid={`cc-legend-${tool}`}>
        {keys.map(k => <span key={k} className="flex items-center gap-1.5">
          <span className={k === "pct" ? "h-0.5 w-3" : "size-2.5 rounded-sm"} style={{ background: COLORS[k] }} />
          {t(SERIES[k])}
        </span>)}
      </div>
      <ChartCard empty={data.length === 0}>
        <ChartContainer config={chartConfig} className="w-full" style={{ height: 220 }}>
          <ComposedChart data={data} margin={{ top: 8, right: 4, bottom: 0, left: 0 }}>
            <CartesianGrid vertical={false} strokeDasharray="3 3" />
            <XAxis dataKey="date" tickFormatter={(d: string) => d.slice(5)} tickLine={false} axisLine={false} tickMargin={8}
              interval={data.length > 15 ? Math.ceil(data.length / 12) : 0} />
            <YAxis yAxisId="tokens" width={56} tickLine={false} axisLine={false} tickFormatter={(v: number) => formatNumber(v)}
              {...(yTicks.length > 0 && { ticks: yTicks, domain: [yTicks[0], yTicks[yTicks.length - 1]] as [number, number] })} />
            {withRatio && <YAxis yAxisId="pct" orientation="right" width={44} domain={[0, 100]} ticks={[0, 25, 50, 75, 100]}
              tickLine={false} axisLine={false} tickFormatter={(v: number) => `${v}%`} />}
            <ChartsTooltip content={<DailyTooltip />} />
            {keys.filter(k => k !== "pct").map(k => <Bar key={k} yAxisId="tokens" dataKey={k} fill={`var(--color-${k})`} radius={[4, 4, 0, 0]} {...drawIn} />)}
            {withRatio && <Line yAxisId="pct" type="monotone" dataKey="pct" stroke="var(--color-pct)" strokeWidth={2} dot={data.length <= 60} {...drawIn} />}
          </ComposedChart>
        </ChartContainer>
      </ChartCard>
    </div>;
  };
  const rtkChart = dailyChart("rtk", rtkRange.map(r => ({ date: r.date, used: r.input_tokens, compressed: r.saved_tokens, pct: r.savings_pct })));
  const sqzChart = dailyChart("sqz", sqzRange.map(r => ({ date: r.date, compressed: r.saved_tokens })));

  const rtkIn = rtkRange.reduce((a, r) => a + r.input_tokens, 0);
  const rtkSaved = sum(rtkRange);
  const rtkTiles: [string, string, string, "range" | "total"][] | null = statsState === "ok" && stats?.rtk ? [
    ["saved", "contextCompress.statsSaved", formatInteger(rtkSaved), "range"],
    ["pct", "contextCompress.statsPct", rtkIn > 0 ? formatPercent(rtkSaved / rtkIn * 100) : "—", "range"],
    ["count", "contextCompress.statsCommands", formatInteger(rtkRange.reduce((a, r) => a + r.commands, 0)), "range"],
    ["in", "contextCompress.statsIn", formatInteger(rtkIn), "range"],
    ["out", "contextCompress.statsOut", formatInteger(rtkRange.reduce((a, r) => a + r.output_tokens, 0)), "range"],
  ] : null;
  const sqz = stats?.sqz;
  const sqzTiles: [string, string, string, "range" | "total"][] | null = statsState === "ok" && sqz ? [
    ["saved", "contextCompress.statsSaved", formatInteger(sum(sqzRange)), "range"],
    ["pct", "contextCompress.statsPct", formatPercent(sqz.avgReduction), "total"],
    ["count", "contextCompress.statsCompressions", formatInteger(sqz.totalCompressions), "total"],
    ["in", "contextCompress.statsIn", formatInteger(sqz.tokensIn), "total"],
    ["out", "contextCompress.statsOut", formatInteger(sqz.tokensOut), "total"],
  ] : null;

  return (
    <section className="space-y-5" data-testid="context-compress-page">
      <header>
        <h1 className="text-xl font-semibold">{t("contextCompress.title")}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{t("contextCompress.desc")}</p>
      </header>
      {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
      <Tabs defaultValue="rtk">
        <TabsList>
          <TabsTrigger value="rtk" data-testid="cc-tab-rtk">RTK</TabsTrigger>
          <TabsTrigger value="sqz" data-testid="cc-tab-sqz">sqz</TabsTrigger>
        </TabsList>
        <TabsContent value="rtk" className="space-y-4">
          <p className="text-sm text-muted-foreground">{t("contextCompress.rtkDesc")}</p>
          <p className="text-sm" data-testid="rtk-version" data-version={status?.rtk_version ?? ""} data-state={!status ? "loading" : status.rtk_version ? "installed" : "missing"}>{label(status?.rtk_version ?? null)}</p>
          {status && !status.rtk_version && <Button disabled={busy} onClick={() => void install("rtk")} data-testid="rtk-install">{t("contextCompress.install")}</Button>}
          {status?.rtk_version && <div className="space-y-3">
            <label className="flex items-center justify-between gap-4 rounded-lg border p-4">
              <span>RTK</span>
              <Switch checked={status.rtk_claude || status.rtk_codex} disabled={busy} onCheckedChange={on => void run(() => contextCompressApi.setRtk(on))} aria-label="RTK" data-testid="rtk-toggle" />
            </label>
            <div className="flex flex-col gap-1 px-1 text-sm text-muted-foreground">
              <span data-testid="rtk-side-claude-code">Claude Code: {status.rtk_claude ? t("plugins.enabled") : t("plugins.disabled")}</span>
              <span data-testid="rtk-side-codex">Codex: {status.rtk_codex ? t("plugins.enabled") : t("plugins.disabled")}{!status.codex_present && ` · ${t("contextCompress.codexMissing")}`}</span>
              {sideErrors.claude && <span className="text-destructive" data-testid="rtk-error-claude-code">{sideErrors.claude}</span>}
              {sideErrors.codex && <span className="text-destructive" data-testid="rtk-error-codex">{sideErrors.codex}</span>}
            </div>
          </div>}
          {card("rtk", rtkTiles, rtkChart)}
        </TabsContent>
        <TabsContent value="sqz" className="space-y-4">
          <p className="text-sm text-muted-foreground">{t("contextCompress.sqzDesc")}</p>
          <p className="text-sm" data-testid="sqz-version">sqz: {label(status?.sqz_version ?? null)} · sqz-mcp: {label(status?.sqz_mcp_version ?? null)}</p>
          {status && (!status.sqz_version || !status.sqz_mcp_version) && <Button disabled={busy} onClick={() => void install("sqz")} data-testid="sqz-install">{t("contextCompress.install")}</Button>}
          {status?.sqz_version && status.sqz_mcp_version && <label className="flex items-center justify-between gap-4 rounded-lg border p-4">
            <span>{t("contextCompress.sqzSwitch")}</span>
            <Switch checked={status.sqz_enabled} disabled={busy} onCheckedChange={on => void run(() => contextCompressApi.setSqz(on))} aria-label={t("contextCompress.sqzSwitch")} data-testid="sqz-toggle" />
          </label>}
          {!!status?.sqz_missing.length && <div className="text-sm text-muted-foreground" data-testid="sqz-missing">
            <p>{t("contextCompress.sqzMissing")}</p>
            <ul className="list-disc pl-5">{status.sqz_missing.map(item => <li key={item}>{item}</li>)}</ul>
          </div>}
          {card("sqz", sqzTiles, sqzChart, <p className="text-sm text-muted-foreground" data-testid="cc-sqz-rate-note">{t("contextCompress.sqzRateNote")}</p>)}
        </TabsContent>
      </Tabs>
    </section>
  );
}
