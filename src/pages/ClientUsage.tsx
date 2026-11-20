// ClientUsage.tsx — 日志统计 → 客户端统计（issue #65 票 02b）。
// 同意闸门：首次进入（asked=false）只见同意卡；保存后进入主视图（开关可随时改）。

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { Switch } from "../components/ui/switch";
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
  type ClientUsageConsent,
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

export function ClientUsage() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ClientUsageStatus | null>(null);
  const [consentDraft, setConsentDraft] = useState<ClientUsageConsent>({ asked: false, enabled: {} });
  const [consentBusy, setConsentBusy] = useState(false);
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

  const asked = status?.consent.asked ?? false;
  const enabled = status?.consent.enabled ?? {};
  const importing = status?.import.running ?? false;

  const range = useMemo(
    () => presetRange(preset, fromDate, toDate),
    [preset, fromDate, toDate],
  );

  const loadStatus = useCallback(async () => {
    try {
      const s = await clientUsageApi.status();
      setStatus(s);
      setConsentDraft(s.consent);
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
    if (asked) void loadData(range);
  }, [asked, range, loadData]);

  // 导入进行中轮询 status；结束后刷一次数据。
  useEffect(() => {
    if (!importing) return;
    const timer = setTimeout(() => { void loadStatus(); }, 1500);
    return () => clearTimeout(timer);
  }, [importing, status, loadStatus]);

  const saveConsent = async () => {
    setConsentBusy(true);
    setError("");
    try {
      await clientUsageApi.consentSet({ ...consentDraft, asked: true });
      await loadStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setConsentBusy(false);
    }
  };

  const toggleClient = async (code: string, on: boolean) => {
    setError("");
    const next: ClientUsageConsent = { asked: true, enabled: { ...enabled, [code]: on } };
    // 开关即时生效：乐观更新，失败回滚并报错。
    setStatus((s) => (s ? { ...s, consent: next } : s));
    try {
      await clientUsageApi.consentSet(next);
    } catch (e) {
      setError(String(e));
      void loadStatus();
    }
  };

  const refresh = async () => {
    setError("");
    try {
      const st = await clientUsageApi.refresh();
      setStatus((s) => (s ? { ...s, import: st } : s));
    } catch (e) {
      setError(String(e));
    }
  };

  const setEnabled = (code: string) => (on: boolean) => { void toggleClient(code, on); };

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

  if (!asked) {
    // 首次进入：同意卡。
    return (
      <section className="space-y-5" data-testid="client-usage-page">
        <header>
          <h1 className="text-xl font-semibold">{t("clientUsage.title", "客户端统计")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("clientUsage.desc", "读取本地 AI 编码客户端的会话文件，按官方 API 价格折算本地用量。数据仅本机处理与存储。")}</p>
        </header>
        <Card className="p-5 space-y-4" data-testid="consent-card">
          <h2 className="font-medium">{t("clientUsage.consentTitle", "读取授权")}</h2>
          <p className="text-sm text-muted-foreground">{t("clientUsage.consentDesc", "选择要统计的客户端。开启后读取对应客户端本地的会话文件并导入统计。")}</p>
          <div className="space-y-3">
            {CLIENTS.map((c) => (
              <label key={c.code} className="flex items-center justify-between rounded-lg border p-3">
                <span className="text-sm">{c.name}</span>
                <Switch
                  checked={consentDraft.enabled[c.code] ?? false}
                  onCheckedChange={(on) => setConsentDraft((d) => ({ ...d, enabled: { ...d.enabled, [c.code]: on } }))}
                  aria-label={c.name}
                  data-testid={`consent-${c.code}`}
                />
              </label>
            ))}
          </div>
          <Button data-testid="consent-save" disabled={consentBusy} onClick={() => void saveConsent()}>
            {t("clientUsage.consentSave", "开始统计")}
          </Button>
        </Card>
        {error && <p role="alert" className="text-sm text-destructive" data-testid="consent-error">{error}</p>}
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

      {/* 客户端开关（随时改，关掉即停止读取）。 */}
      <Card className="p-4" data-testid="client-switches">
        <div className="flex flex-wrap gap-x-8 gap-y-3">
          {CLIENTS.map((c) => (
            <label key={c.code} className="flex items-center gap-2 text-sm">
              <span>{c.name}</span>
              <Switch
                checked={enabled[c.code] ?? false}
                onCheckedChange={setEnabled(c.code)}
                aria-label={c.name}
                data-testid={`client-switch-${c.code}`}
              />
            </label>
          ))}
        </div>
        {(importing || (imp && imp.callsImported > 0) || (imp && !!imp.error) || (status?.persistedParseErrors ?? 0) > 0) && (
          <div className="mt-3 text-sm text-muted-foreground space-y-1" data-testid="import-state">
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
          </div>
        )}
      </Card>

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
