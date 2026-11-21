// Sessions.tsx — 扩展 › 会话（扩展-会话页 票 02，设计稿方向 3「客户端四格」）。
// 数据只用现有命令（client_usage_status / sessions / refresh）；刷新事件驱动：订阅 client-usage-updated，
// 窗口隐藏期间不拉，重新可见补拉；不轮询。运行状态（票 05）随数据一起拉，阈值到期由单次定时器本地重新派生。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../components/ui/select";
import { ClientTiles } from "../components/sessions/ClientTiles";
import { SessionGroups } from "../components/sessions/SessionGroups";
import { SESSION_CLIENTS, clientName } from "../components/sessions/ClientGlyph";
import { deriveState, nextFlipDelay } from "../components/sessions/sessionState";
import { clientUsageApi, type ClientUsageSessionRow, type ClientUsageSessionState, type ClientUsageStatus } from "../services/api/clientUsage";
import { listen } from "../services/transport";

type Range = "today" | "7d" | "30d";

function rangeMs(range: Range): { fromMs: number; toMs: number } {
  const now = new Date();
  const s = new Date(now);
  if (range === "today") s.setHours(0, 0, 0, 0);
  else s.setDate(s.getDate() - (range === "7d" ? 7 : 30));
  return { fromMs: s.getTime(), toMs: now.getTime() };
}

export function Sessions({ onNavigate }: { onNavigate?: (id: string) => void } = {}) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ClientUsageStatus | null>(null);
  const [rows, setRows] = useState<ClientUsageSessionRow[]>([]);
  const [states, setStates] = useState<Record<string, ClientUsageSessionState>>({});
  const [now, setNow] = useState(() => Date.now());
  const [error, setError] = useState("");
  const [range, setRange] = useState<Range>("7d");
  const [pick, setPick] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const enabled = useMemo(() => status?.consent.enabled ?? {}, [status]);
  const anyEnabled = SESSION_CLIENTS.some((c) => enabled[c.code]);
  const goSettings = () => onNavigate?.("settings/client_usage");

  const load = useCallback(async () => {
    setError("");
    try {
      const st = await clientUsageApi.status();
      const on = SESSION_CLIENTS.some((c) => st.consent.enabled[c.code]);
      const { fromMs, toMs } = rangeMs(range);
      const list = on ? await clientUsageApi.sessions(fromMs, toMs) : [];
      // 状态拉取失败不挡列表：徽标回落按最近调用时间派生。
      const ss = list.length ? await clientUsageApi.sessionStates(list.map((r) => r.sessionId)).catch(() => []) : [];
      setStatus(st);
      setRows(list);
      setStates(Object.fromEntries(ss.map((x) => [`${x.clientCode}:${x.sessionId}`, x])));
      setNow(Date.now());
    } catch (e) {
      setError(String(e));
    }
  }, [range]);

  useEffect(() => { void load(); }, [load]);

  // 事件驱动刷新：订阅一次，经 ref 调最新 load（时间范围变更不重订阅）。
  const loadRef = useRef(load);
  loadRef.current = load;
  useEffect(() => {
    let alive = true;
    let unlisten: () => void = () => {};
    listen("client-usage-updated", () => {
      if (document.visibilityState === "visible") void loadRef.current();
    }).then((fn) => { if (alive) unlisten = fn; else fn(); }).catch(() => {});
    const onVisible = () => { if (document.visibilityState === "visible") void loadRef.current(); };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      alive = false;
      unlisten();
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, []);

  // 阈值到期只重新派生（spec S8）：每次只挂一个定时器，指向最近的一次状态翻转。
  useEffect(() => {
    const lasts = rows.flatMap((r) => {
      const s = states[`${r.clientCode}:${r.sessionId}`];
      return s && s.source !== "activity" ? [] : [s ? s.lastActivityMs : r.lastCallMs];
    });
    const delay = nextFlipDelay(lasts, now);
    if (delay === null) return;
    const id = setTimeout(() => setNow(Date.now()), delay);
    return () => clearTimeout(id);
  }, [rows, states, now]);
  const stateOf = (r: ClientUsageSessionRow) => deriveState(states[`${r.clientCode}:${r.sessionId}`], r.lastCallMs, now);

  const rescan = async () => {
    try {
      const imp = await clientUsageApi.refresh();
      setStatus((s) => (s ? { ...s, import: imp } : s));
    } catch (e) {
      setError(String(e));
    }
  };

  // 继续会话（票 07）：macOS 后端直接开终端；其它平台返回命令，复制给用户。
  const [notice, setNotice] = useState<{ kind: "error" | "copied"; text: string } | null>(null);
  const resume = async (r: ClientUsageSessionRow) => {
    setNotice(null);
    try {
      const res = await clientUsageApi.sessionResume(r.clientCode, r.sessionId, r.workdir);
      if (!res.launched) {
        await navigator.clipboard?.writeText(res.command);
        setNotice({ kind: "copied", text: res.command });
      }
    } catch (e) {
      setNotice({ kind: "error", text: String(e) });
    }
  };

  const enabledRows = useMemo(
    () => rows.filter((r) => enabled[r.clientCode]).sort((a, b) => b.lastCallMs - a.lastCallMs),
    [rows, enabled],
  );
  const activePick = pick && enabled[pick] ? pick : null;
  const q = query.trim().toLowerCase();
  const shown = enabledRows.filter((r) =>
    (!activePick || r.clientCode === activePick)
    && (!q || r.title.toLowerCase().includes(q) || r.workdir.toLowerCase().includes(q)));
  const rangeLabel = { today: t("stats.today", "今天"), "7d": t("stats.7d", "近 7 天"), "30d": t("stats.30d", "近 30 天") }[range];
  const importing = status?.import.running ?? false;

  return (
    <section className="space-y-5" data-testid="sessions-page">
      <header className="flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold">{t("sessions.title", "会话")}</h1>
        <span className="text-sm text-muted-foreground">{t("sessions.desc", "本机 AI 客户端会话的用量记录，不含对话内容")}</span>
        <span className="flex-1" />
        <Select value={range} onValueChange={(v) => setRange(v as Range)}>
          <SelectTrigger className="w-36" data-testid="sessions-range"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="today" data-testid="range-opt-today">{t("stats.today", "今天")}</SelectItem>
            <SelectItem value="7d" data-testid="range-opt-7d">{t("stats.7d", "近 7 天")}</SelectItem>
            <SelectItem value="30d" data-testid="range-opt-30d">{t("stats.30d", "近 30 天")}</SelectItem>
          </SelectContent>
        </Select>
        {anyEnabled && (
          <Button variant="outline" data-testid="sessions-refresh" disabled={importing} onClick={() => void rescan()}>
            {importing ? t("clientUsage.refreshing", "正在导入…") : t("clientUsage.refresh", "立即刷新")}
          </Button>
        )}
      </header>

      {error && (
        <div role="alert" className="flex items-center gap-3 text-sm text-destructive" data-testid="sessions-load-error">
          <span className="min-w-0 break-all">{error}</span>
          <Button size="sm" variant="outline" data-testid="sessions-retry" onClick={() => void load()}>{t("sessions.retry", "重试")}</Button>
        </div>
      )}

      {!status && !error && (
        <div className="h-[118px] animate-pulse rounded-xl border bg-card" data-testid="sessions-loading" aria-busy="true" />
      )}

      {status && (
        <ClientTiles
          enabled={enabled}
          rows={enabledRows}
          importState={status.import}
          pick={activePick}
          onPick={(code) => setPick((p) => (p === code ? null : code))}
          onGoSettings={goSettings}
        />
      )}

      {status && !anyEnabled && (
        <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 rounded-xl border border-dashed bg-card p-10" data-testid="sessions-guide">
          <div>
            <h2 className="mb-2 text-xl font-semibold">{t("sessions.guideTitle", "四个客户端都还没有开启读取")}</h2>
            <p className="max-w-[620px] leading-relaxed text-muted-foreground">{t("sessions.guideDesc", "开启后，AiDog 读取所选客户端在本机写下的会话文件，按项目列出每个会话的调用次数、token 与成本。读取开关在设置里，可以只开其中一个。")}</p>
            <ul className="mt-3 list-disc ps-5 text-sm leading-loose text-muted-foreground">
              <li>{t("sessions.guideMeta", "只记录元数据：会话标题、工作目录、token、成本")}</li>
              <li>{t("sessions.guideNoBody", "对话正文不入库；随时可在设置中关闭")}</li>
            </ul>
          </div>
          <Button data-testid="sessions-guide-go" onClick={goSettings}>{t("clientUsage.guideGo", "去设置启用")}</Button>
        </div>
      )}

      {status && anyEnabled && (
        <>
          <div className="flex items-center gap-2.5" data-testid="sessions-toolbar">
            <span className="text-muted-foreground" data-testid="sessions-filter-label">
              {activePick ? <b className="font-medium text-foreground">{clientName(activePick)}</b> : t("sessions.allClients", "全部客户端")}
              {" · "}
              <span className="font-mono tabular-nums" data-testid="sessions-count" data-count={String(shown.length)}>
                {t("sessions.count", "{{n}} 个会话", { n: shown.length })}
              </span>
            </span>
            {activePick && (
              <Button variant="ghost" size="sm" data-testid="sessions-clear-filter" onClick={() => setPick(null)}>
                {t("sessions.clearFilter", "清除筛选")}
              </Button>
            )}
            <span className="flex-1" />
            <Input
              className="w-64"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("sessions.search", "搜索标题或目录")}
              aria-label={t("sessions.search", "搜索标题或目录")}
              data-testid="sessions-search"
            />
          </div>

          {enabledRows.length === 0 ? (
            <div className="border-t py-10 text-muted-foreground" data-testid="sessions-empty">
              <h3 className="mb-1.5 text-lg font-semibold text-foreground">{t("sessions.emptyTitle", "{{range}}没有会话记录", { range: rangeLabel })}</h3>
              <p className="mb-3.5">{t("sessions.emptyDesc", "已启用的客户端暂时没有新会话。会话文件更新后会自动导入。")}</p>
              <Button variant="outline" data-testid="sessions-rescan" disabled={importing} onClick={() => void rescan()}>
                {t("sessions.rescan", "重新扫描")}
              </Button>
            </div>
          ) : shown.length === 0 ? (
            <p className="border-t py-10 text-muted-foreground" data-testid="sessions-no-match">{t("sessions.noMatch", "没有匹配的会话")}</p>
          ) : (
            <>
              {notice && (
                <div
                  role={notice.kind === "error" ? "alert" : "status"}
                  className={`text-sm break-all ${notice.kind === "error" ? "text-destructive" : "text-muted-foreground"}`}
                  data-testid="sessions-resume-notice"
                  data-kind={notice.kind}
                  data-detail={notice.text}
                >
                  {notice.kind === "error"
                    ? t("sessions.resumeFailed", "继续会话失败：{{error}}", { error: notice.text })
                    : t("sessions.resumeCopied", "当前系统无法直接打开终端，继续命令已复制：{{command}}", { command: notice.text })}
                </div>
              )}
              <SessionGroups rows={shown} stateOf={stateOf} onResume={(r) => void resume(r)} />
              <p className="text-sm text-muted-foreground">{t("sessions.footHint", "点客户端格可筛选。成本带 + 表示部分调用未定价。")}</p>
            </>
          )}
        </>
      )}
    </section>
  );
}
