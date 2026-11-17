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
  const [error, setError] = useState("");
  const [sideErrors, setSideErrors] = useState<{ claude?: string | null; codex?: string | null }>({});
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [status, stats] = await Promise.all([
        contextCompressApi.status(),
        contextCompressApi.stats().catch(() => null),
      ]);
      setStatus(status);
      setStats(stats);
      setError("");
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

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
  const label = (version: string | null) => version
    ? t("contextCompress.installed", { version })
    : t("contextCompress.notInstalled");

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
          <p className="text-sm" data-testid="rtk-version" data-version={status?.rtk_version ?? ""}>{label(status?.rtk_version ?? null)}</p>
          {!status?.rtk_version && <Button disabled={busy} onClick={() => void install("rtk")} data-testid="rtk-install">{t("contextCompress.install")}</Button>}
          {status?.rtk_version && stats?.rtk && <p className="text-sm text-muted-foreground" data-testid="rtk-stats">
            {t("contextCompress.rtkStats", {
              saved: formatInteger(stats.rtk.total_saved),
              commands: formatInteger(stats.rtk.total_commands),
              pct: formatPercent(stats.rtk.avg_savings_pct),
            })}
          </p>}
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
          {status?.sqz_version && stats?.sqz && <p className="text-sm text-muted-foreground" data-testid="sqz-stats">
            {t("contextCompress.sqzStats", {
              count: formatInteger(stats.sqz.totalCompressions),
              saved: formatInteger(stats.sqz.tokensSaved),
              pct: formatPercent(stats.sqz.avgReduction),
            })}
          </p>}
          {(!status?.sqz_version || !status?.sqz_mcp_version) && <Button disabled={busy} onClick={() => void install("sqz")} data-testid="sqz-install">{t("contextCompress.install")}</Button>}
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
