// Sessions.tsx — 扩展 › 会话（扩展-会话页 票 02；票 11：四格改为客户端 tab，正在执行区置顶）。
// 数据只用现有命令（client_usage_status / sessions / refresh）；刷新事件驱动：订阅 client-usage-updated，
// 窗口隐藏期间不拉，重新可见补拉；不轮询。运行状态（票 05）随数据一起拉，阈值到期由单次定时器本地重新派生。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../components/ui/select";
import { Trash2, X } from "lucide-react";
import { createPortal } from "react-dom";
import { SessionGroups } from "../components/sessions/SessionGroups";
import { SessionTrash } from "../components/sessions/SessionTrash";
import { ClientIcon, SESSION_CLIENTS } from "../components/sessions/ClientIcon";
import { deriveState, nextFlipDelay } from "../components/sessions/sessionState";
import { clientUsageApi, type ClientUsageSessionRow, type ClientUsageSessionState, type ClientUsageStatus } from "../services/api/clientUsage";
import { listen } from "../services/transport";
import { homeDir, revealItemInDir } from "../services/platform";

type Range = "today" | "7d" | "30d";

function rangeMs(range: Range): { fromMs: number; toMs: number } {
  const now = new Date();
  const s = new Date(now);
  if (range === "today") s.setHours(0, 0, 0, 0);
  else s.setDate(s.getDate() - (range === "7d" ? 7 : 30));
  return { fromMs: s.getTime(), toMs: now.getTime() };
}

const sameRow = (a: ClientUsageSessionRow, b: ClientUsageSessionRow) =>
  (Object.keys(b) as (keyof ClientUsageSessionRow)[]).every((k) => a[k] === b[k]);

/** 重拉后字段没变的行沿用旧对象（票 12）：memo 行组件据此跳过重渲染。 */
function reuseUnchanged(prev: ClientUsageSessionRow[], next: ClientUsageSessionRow[]): ClientUsageSessionRow[] {
  const old = new Map(prev.map((r) => [`${r.clientCode}:${r.sessionId}`, r]));
  return next.map((n) => {
    const o = old.get(`${n.clientCode}:${n.sessionId}`);
    return o && sameRow(o, n) ? o : n;
  });
}

/** 事件重拉合并窗口（票 12）：活跃会话期间事件约每 7 秒一次，连发时只拉一次。 */
const RELOAD_DEBOUNCE_MS = 1500;

export function Sessions({ onNavigate }: { onNavigate?: (id: string) => void } = {}) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ClientUsageStatus | null>(null);
  const [rows, setRows] = useState<ClientUsageSessionRow[]>([]);
  const [states, setStates] = useState<Record<string, ClientUsageSessionState>>({});
  const [now, setNow] = useState(() => Date.now());
  const [error, setError] = useState("");
  const [range, setRange] = useState<Range>("7d");
  const [tab, setTab] = useState<string>("all");
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
      setRows((prev) => reuseUnchanged(prev, list));
      setStates(Object.fromEntries(ss.map((x) => [`${x.clientCode}:${x.sessionId}`, x])));
      setNow(Date.now());
    } catch (e) {
      setError(String(e));
    }
  }, [range]);

  useEffect(() => { void load(); }, [load]);

  // 换客户端继续（票 10）：本机已安装客户端与「允许读取对话原文」开关，进页拉一次；失败按未安装 / 未开处理。
  const [installed, setInstalled] = useState<string[]>([]);
  const [transcriptOn, setTranscriptOn] = useState(false);
  useEffect(() => {
    let alive = true;
    clientUsageApi.installedClients().then((v) => { if (alive) setInstalled(v); }).catch(() => {});
    clientUsageApi.transcriptConsentGet().then((v) => { if (alive) setTranscriptOn(v); }).catch(() => {});
    return () => { alive = false; };
  }, []);

  // 事件驱动刷新：订阅一次，经 ref 调最新 load（时间范围变更不重订阅）。可见时 trailing 防抖合并；
  // 隐藏期间只记「漏过事件」，重新可见才补拉——没漏事件时切回窗口（如打开目录后从 Finder 回来）不重拉。
  const loadRef = useRef(load);
  loadRef.current = load;
  useEffect(() => {
    let alive = true;
    let unlisten: () => void = () => {};
    let timer: ReturnType<typeof setTimeout> | undefined;
    let missed = false;
    listen("client-usage-updated", () => {
      if (document.visibilityState !== "visible") { missed = true; return; }
      clearTimeout(timer);
      timer = setTimeout(() => {
        if (document.visibilityState === "visible") void loadRef.current();
        else missed = true;
      }, RELOAD_DEBOUNCE_MS);
    }).then((fn) => { if (alive) unlisten = fn; else fn(); }).catch(() => {});
    const onVisible = () => {
      if (document.visibilityState !== "visible" || !missed) return;
      missed = false;
      void loadRef.current();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      alive = false;
      clearTimeout(timer);
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

  // 继续会话（票 14）：一律复制整行命令（含 cd），不开终端；换客户端继续仍是 macOS 开终端、其它平台复制。
  const [notice, setNotice] = useState<{ kind: "error" | "copied" | "resume-copied" | "open-error" | "hidden" | "hide-error"; text: string; row?: ClientUsageSessionRow } | null>(null);
  const [trashOpen, setTrashOpen] = useState(false);
  // 继续请求在途的会话（票 12）：返回前禁用按钮；ref 挡住同一帧内的连点。
  const resumingRef = useRef(new Set<string>());
  const [resuming, setResuming] = useState<ReadonlySet<string>>(() => new Set());
  // to 非空 = 换客户端继续（票 10），结果与继续同形。
  const resume = useCallback(async (r: ClientUsageSessionRow, to?: string) => {
    const key = `${r.clientCode}:${r.sessionId}`;
    if (resumingRef.current.has(key)) return;
    resumingRef.current.add(key);
    setResuming(new Set(resumingRef.current));
    setNotice(null);
    try {
      const res = to
        ? await clientUsageApi.sessionCarry(r.clientCode, r.sessionId, r.workdir, to)
        : await clientUsageApi.sessionResume(r.clientCode, r.sessionId, r.workdir);
      if (!to || !res.launched) {
        await navigator.clipboard?.writeText(res.command);
        setNotice({ kind: to ? "copied" : "resume-copied", text: res.command });
      }
    } catch (e) {
      setNotice({ kind: "error", text: String(e) });
    } finally {
      resumingRef.current.delete(key);
      setResuming(new Set(resumingRef.current));
    }
  }, []);
  const isResuming = useCallback((r: ClientUsageSessionRow) => resuming.has(`${r.clientCode}:${r.sessionId}`), [resuming]);

  // 打开工作目录（票 08）：在父目录中选中该目录（spec S10）；浏览器形态退化为复制路径。
  const openDir = useCallback(async (workdir: string) => {
    setNotice(null);
    try {
      let p = workdir;
      if (p === "~" || p.startsWith("~/")) {
        const home = await homeDir();
        if (home) p = home.replace(/[\\/]+$/, "") + p.slice(1);
      }
      // 路径来自会话文件，不可信：拒绝选项样式与控制字符。
      // biome-ignore lint/suspicious/noControlCharactersInRegex: 正是要拦控制字符
      if (p.startsWith("-") || /[\u0000-\u001f\u007f]/.test(p)) throw `invalid path: ${workdir}`;
      await revealItemInDir(p);
    } catch (e) {
      setNotice({ kind: "open-error", text: String(e) });
    }
  }, []);

  // 从列表隐藏（票 09）：无确认，成功后本地移除并给出撤销；不删任何客户端文件。
  const hide = useCallback(async (r: ClientUsageSessionRow) => {
    setNotice(null);
    try {
      await clientUsageApi.sessionHide(r.clientCode, r.sessionId);
      setRows((rs) => rs.filter((x) => !(x.clientCode === r.clientCode && x.sessionId === r.sessionId)));
      setNotice({ kind: "hidden", text: r.title || r.sessionId, row: r });
    } catch (e) {
      setNotice({ kind: "hide-error", text: String(e) });
    }
  }, []);
  const onResume = useCallback((r: ClientUsageSessionRow) => void resume(r), [resume]);
  const onCarry = useCallback((r: ClientUsageSessionRow, to: string) => void resume(r, to), [resume]);
  const onNeedTranscript = useCallback(() => onNavigate?.("settings/client_usage"), [onNavigate]);
  const onOpenDir = useCallback((d: string) => void openDir(d), [openDir]);
  const onHide = useCallback((r: ClientUsageSessionRow) => void hide(r), [hide]);
  const undoHide = async (r: ClientUsageSessionRow) => {
    try {
      await clientUsageApi.sessionUnhide(r.clientCode, r.sessionId);
      setNotice(null);
      await load();
    } catch (e) {
      setNotice({ kind: "hide-error", text: String(e) });
    }
  };

  const enabledRows = useMemo(
    () => rows.filter((r) => enabled[r.clientCode]).sort((a, b) => b.lastCallMs - a.lastCallMs),
    [rows, enabled],
  );
  const activeTab = tab !== "all" && enabled[tab] ? tab : "all";
  const q = query.trim().toLowerCase();
  const shown = enabledRows.filter((r) =>
    (activeTab === "all" || r.clientCode === activeTab)
    && (!q || r.title.toLowerCase().includes(q) || r.workdir.toLowerCase().includes(q)));
  // 正在执行区（票 11）：任一客户端执行中 + Claude Code pid 文件在（窗口未关，busy / idle 都算）；按最近活动倒序。
  const activityOf = (r: ClientUsageSessionRow) => Math.max(r.lastCallMs, states[`${r.clientCode}:${r.sessionId}`]?.lastActivityMs ?? 0);
  const pinned = shown
    .filter((r) => stateOf(r) === "running" || (r.clientCode === "claude_code" && states[`${r.clientCode}:${r.sessionId}`]?.source === "pid_file"))
    .sort((a, b) => activityOf(b) - activityOf(a));
  const offClients = SESSION_CLIENTS.filter((c) => !enabled[c.code]);
  const rangeLabel = { today: t("stats.today", "Today"), "7d": t("stats.7d", "Last 7d"), "30d": t("stats.30d", "Last 30d") }[range];
  const importing = status?.import.running ?? false;

  return (
    <section className="space-y-5" data-testid="sessions-page">
      <header className="flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold">{t("sessions.title", "Sessions")}</h1>
        <span className="text-sm text-muted-foreground">{t("sessions.desc", "Usage records of local AI client sessions, without conversation content")}</span>
        <span className="flex-1" />
        <Select value={range} onValueChange={(v) => setRange(v as Range)}>
          <SelectTrigger className="w-36" data-testid="sessions-range"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="today" data-testid="range-opt-today">{t("stats.today", "Today")}</SelectItem>
            <SelectItem value="7d" data-testid="range-opt-7d">{t("stats.7d", "Last 7d")}</SelectItem>
            <SelectItem value="30d" data-testid="range-opt-30d">{t("stats.30d", "Last 30d")}</SelectItem>
          </SelectContent>
        </Select>
        {anyEnabled && (
          <Button variant="outline" data-testid="sessions-refresh" disabled={importing} onClick={() => void rescan()}>
            {importing ? t("clientUsage.refreshing", "Importing…") : t("clientUsage.refresh", "Refresh Now")}
          </Button>
        )}
      </header>

      {error && (
        <div role="alert" className="flex items-center gap-3 text-sm text-destructive" data-testid="sessions-load-error">
          <span className="min-w-0 break-all">{error}</span>
          <Button size="sm" variant="outline" data-testid="sessions-retry" onClick={() => void load()}>{t("sessions.retry", "Retry")}</Button>
        </div>
      )}

      {!status && !error && (
        <div className="h-[118px] animate-pulse rounded-xl border bg-card" data-testid="sessions-loading" aria-busy="true" />
      )}

      {status && !anyEnabled && (
        <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-8 rounded-xl border border-dashed bg-card p-10" data-testid="sessions-guide">
          <div>
            <h2 className="mb-2 text-xl font-semibold">{t("sessions.guideTitle", "None of the four clients has reading enabled")}</h2>
            <p className="max-w-[620px] leading-relaxed text-muted-foreground">{t("sessions.guideDesc", "Once enabled, AiDog reads the session files the selected clients write on this machine and lists each session's calls, tokens and cost by project. The reading switches are in Settings; you can enable just one.")}</p>
            <ul className="mt-3 list-disc ps-5 text-sm leading-loose text-muted-foreground">
              <li>{t("sessions.guideMeta", "Only metadata is recorded: session title, working directory, tokens, cost")}</li>
              <li>{t("sessions.guideNoBody", "Conversation text is never stored; you can turn it off in Settings at any time")}</li>
            </ul>
          </div>
          <Button data-testid="sessions-guide-go" onClick={goSettings}>{t("clientUsage.guideGo", "Enable in Settings")}</Button>
        </div>
      )}

      {status && anyEnabled && (
        <>
          {offClients.length > 0 && (
            <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground" data-testid="sessions-partial-off" data-clients={offClients.map((c) => c.code).join(",")}>
              {offClients.map((c) => (
                <span key={c.code} className="inline-flex items-center gap-1 text-foreground">
                  <ClientIcon code={c.code} size={14} />{c.name}
                </span>
              ))}
              <span>{t("sessions.tileOff", "Reading off")} ·</span>
              <Button variant="link" size="sm" className="h-auto p-0" data-testid="sessions-partial-off-go" onClick={goSettings}>
                {t("clientUsage.guideGo", "Enable in Settings")}
              </Button>
            </div>
          )}

          {/* 窄窗：放不下时回收站 + 搜索框换到第二行；搜索框不超行宽 */}
          <div className="flex flex-wrap items-center gap-2.5" data-testid="sessions-toolbar">
            <div role="tablist" className="flex min-w-0 max-w-full flex-wrap items-center gap-1 rounded-lg border border-border bg-card p-1" aria-label={t("sessions.title", "Sessions")}>
              {[{ code: "all", name: t("sessions.tabAll", "All") }, ...SESSION_CLIENTS].map((c) => {
                const off = c.code !== "all" && !enabled[c.code];
                const selected = activeTab === c.code;
                return (
                  <button
                    key={c.code}
                    type="button"
                    role="tab"
                    aria-selected={selected}
                    disabled={off}
                    title={off ? t("sessions.tileOff", "Reading off") : undefined}
                    className={`inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-md px-3 py-1.5 text-sm ${selected ? "bg-[var(--accent-subtle)] font-medium text-foreground" : "text-muted-foreground hover:bg-muted"} disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent`}
                    data-testid={`sessions-tab-${c.code}`}
                    onClick={() => setTab(c.code)}
                  >
                    {c.code !== "all" && <ClientIcon code={c.code} size={14} />}
                    {c.name}
                  </button>
                );
              })}
            </div>
            <span className="font-mono text-sm text-muted-foreground tabular-nums" data-testid="sessions-count" data-count={String(shown.length)}>
              {t("sessions.count", "{{n}} sessions", { n: shown.length })}
            </span>
            <span className="flex-1" />
            <Button variant="outline" data-testid="sessions-trash" onClick={() => setTrashOpen(true)}>
              <Trash2 aria-hidden className="size-4" />
              {t("sessions.trash", "Trash")}
            </Button>
            <Input
              className="w-64 max-w-full"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("sessions.search", "Search title or directory")}
              aria-label={t("sessions.search", "Search title or directory")}
              data-testid="sessions-search"
            />
          </div>

          {/* 行内操作提示（票 12）：portal 到 body 的视口固定浮层，滚到列表深处点击也看得见；隐藏最后一个会话后撤销仍可见 */}
          {notice && createPortal(
            <div className="pointer-events-none fixed inset-x-0 bottom-6 z-50 flex justify-center px-4" data-testid="sessions-notice-layer">
              <div
                role={notice.kind === "copied" || notice.kind === "resume-copied" || notice.kind === "hidden" ? "status" : "alert"}
                className={`pointer-events-auto flex max-w-[min(720px,100%)] items-center gap-3 rounded-lg border bg-card px-4 py-2.5 text-sm shadow-lg break-all ${notice.kind === "copied" || notice.kind === "resume-copied" || notice.kind === "hidden" ? "text-foreground" : "border-destructive/40 text-destructive"}`}
                data-testid="sessions-resume-notice"
                data-kind={notice.kind}
                data-detail={notice.text}
              >
                <span className="min-w-0">
                  {notice.kind === "error"
                    ? t("sessions.resumeFailed", "Failed to resume session: {{error}}", { error: notice.text })
                    : notice.kind === "open-error"
                    ? t("sessions.openDirFailed", "Couldn't open folder: {{error}}", { error: notice.text })
                    : notice.kind === "hide-error"
                    ? t("sessions.hideFailed", "Couldn't hide: {{error}}", { error: notice.text })
                    : notice.kind === "hidden"
                    ? t("sessions.hidden", "Hidden from list: {{title}}", { title: notice.text })
                    : notice.kind === "resume-copied"
                    ? t("sessions.resumeCommandCopied", "Resume command copied: {{command}}", { command: notice.text })
                    : t("sessions.resumeCopied", "Can't open a terminal on this system. Resume command copied: {{command}}", { command: notice.text })}
                </span>
                {notice.kind === "hidden" && notice.row && (
                  <Button size="sm" variant="link" className="h-auto shrink-0 p-0" data-testid="sessions-hide-undo" onClick={() => void undoHide(notice.row!)}>
                    {t("sessions.undo", "Undo")}
                  </Button>
                )}
                <Button
                  size="icon"
                  variant="ghost"
                  className="size-7 shrink-0 text-muted-foreground"
                  aria-label={t("sessions.noticeClose", "Dismiss")}
                  title={t("sessions.noticeClose", "Dismiss")}
                  data-testid="sessions-notice-close"
                  onClick={() => setNotice(null)}
                >
                  <X aria-hidden className="size-4" />
                </Button>
              </div>
            </div>,
            document.body,
          )}

          {enabledRows.length === 0 ? (
            <div className="border-t py-10 text-muted-foreground" data-testid="sessions-empty">
              <h3 className="mb-1.5 text-lg font-semibold text-foreground">{t("sessions.emptyTitle", "No sessions — {{range}}", { range: rangeLabel })}</h3>
              <p className="mb-3.5">{t("sessions.emptyDesc", "Enabled clients have no new sessions yet. Updated session files are imported automatically.")}</p>
              <Button variant="outline" data-testid="sessions-rescan" disabled={importing} onClick={() => void rescan()}>
                {t("sessions.rescan", "Rescan")}
              </Button>
            </div>
          ) : shown.length === 0 ? (
            <p className="border-t py-10 text-muted-foreground" data-testid="sessions-no-match">{t("sessions.noMatch", "No matching sessions")}</p>
          ) : (
            <>
              <SessionGroups rows={shown} pinned={pinned} stateOf={stateOf} isResuming={isResuming} onResume={onResume} onOpenDir={onOpenDir} onHide={onHide} installed={installed} transcriptOn={transcriptOn} onCarry={onCarry} onNeedTranscript={onNeedTranscript} />
              <p className="text-sm text-muted-foreground">{t("sessions.footHint", "A + on cost means some calls are unpriced.")}</p>
            </>
          )}
        </>
      )}
      <SessionTrash open={trashOpen} onOpenChange={setTrashOpen} onRestored={() => void load()} />
    </section>
  );
}
