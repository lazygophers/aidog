// ─── 扩展 → 上下文压缩（RTK / sqz）────────────────────────────
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import { Switch } from "../components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../components/ui/tabs";
import { contextCompressApi, type ContextCompressStats, type ContextCompressStatus, type RtkSetResult } from "../services/api/contextCompress";
import { formatInteger, formatPercent } from "../utils/formatters";

export function ContextCompress() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ContextCompressStatus | null>(null);
  const [stats, setStats] = useState<ContextCompressStats | null>(null);
  const [statsState, setStatsState] = useState<"loading" | "ok" | "error">("loading");
  const [statsError, setStatsError] = useState("");
  const [error, setError] = useState("");
  const [sideErrors, setSideErrors] = useState<{ claude?: string | null; codex?: string | null }>({});
  const [busy, setBusy] = useState(false);

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
  const statsCards = [
    {
      tool: "rtk", name: "RTK", version: status?.rtk_version,
      tiles: stats?.rtk && [
        ["saved", "contextCompress.statsSaved", formatInteger(stats.rtk.total_saved)],
        ["pct", "contextCompress.statsPct", formatPercent(stats.rtk.avg_savings_pct)],
        ["count", "contextCompress.statsCommands", formatInteger(stats.rtk.total_commands)],
        ["in", "contextCompress.statsIn", formatInteger(stats.rtk.total_input)],
        ["out", "contextCompress.statsOut", formatInteger(stats.rtk.total_output)],
      ],
    },
    {
      tool: "sqz", name: "sqz", version: status?.sqz_version,
      tiles: stats?.sqz && [
        ["saved", "contextCompress.statsSaved", formatInteger(stats.sqz.tokensSaved)],
        ["pct", "contextCompress.statsPct", formatPercent(stats.sqz.avgReduction)],
        ["count", "contextCompress.statsCompressions", formatInteger(stats.sqz.totalCompressions)],
        ["in", "contextCompress.statsIn", formatInteger(stats.sqz.tokensIn)],
        ["out", "contextCompress.statsOut", formatInteger(stats.sqz.tokensOut)],
      ],
    },
  ] as const;

  return (
    <section className="space-y-5" data-testid="context-compress-page">
      <header>
        <h1 className="text-xl font-semibold">{t("contextCompress.title")}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{t("contextCompress.desc")}</p>
      </header>
      {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
      <section className="space-y-3" data-testid="cc-stats">
        <div className="flex items-center justify-between gap-4">
          <h2 className="text-base font-semibold">{t("contextCompress.statsTitle")}</h2>
          <Button variant="outline" size="sm" disabled={statsState === "loading"} onClick={() => void loadStats()} data-testid="cc-stats-refresh">{t("contextCompress.statsRefresh")}</Button>
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          {statsCards.map(card => {
            const state = statsState !== "ok" ? statsState
              : card.tiles ? "ok"
              : status && !card.version ? "not-installed"
              : "empty";
            return <div key={card.tool} className="rounded-lg border p-4" data-testid={`cc-stats-${card.tool}`} data-state={state}>
              <h3 className="mb-3 text-sm font-medium">{card.name}</h3>
              {state === "ok" && card.tiles
                ? <dl className="grid grid-cols-2 gap-3 sm:grid-cols-3">
                  {card.tiles.map(([key, labelKey, value]) => <div key={key}>
                    <dt className="text-sm text-muted-foreground">{t(labelKey)}</dt>
                    <dd className="text-lg font-semibold tabular-nums" data-testid={`cc-stat-${card.tool}-${key}`}>{value}</dd>
                  </div>)}
                </dl>
                : <p className={`text-sm ${state === "error" ? "text-destructive" : "text-muted-foreground"}`} data-testid={`cc-stats-${card.tool}-msg`}>
                  {state === "loading" ? t("contextCompress.statsLoading")
                    : state === "error" ? `${t("contextCompress.statsError")} ${statsError}`
                    : state === "not-installed" ? t("contextCompress.statsNotInstalled")
                    : t("contextCompress.statsEmpty")}
                </p>}
            </div>;
          })}
        </div>
      </section>
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
        </TabsContent>
      </Tabs>
    </section>
  );
}
