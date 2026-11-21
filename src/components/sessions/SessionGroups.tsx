// 会话列表（设计稿方向 3）：最上方「正在执行」区平铺（票 11），其余按工作目录分组，左列 sticky 项目信息，右列会话行。
// 置顶会话不在目录组重复显示，但目录组的会话数与成本仍计入。操作列：继续（票 07）；打开目录 / 隐藏等由票 08–10 接入。
import { Fragment, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown } from "lucide-react";
import type { ClientUsageSessionRow, SessionRunState } from "../../services/api/clientUsage";
import { formatNumber, formatRelativeTime } from "../../utils/formatters";
import { ClientIcon, clientName } from "./ClientIcon";
import { SessionCost } from "./SessionCost";
import { SessionDetail } from "./SessionDetail";
import { Button } from "../ui/button";

const ROW_GRID = "grid grid-cols-[20px_minmax(0,1fr)_210px_96px_96px_70px] items-center gap-3";

function projectName(workdir: string): string {
  return workdir.split(/[\\/]/).filter(Boolean).pop() ?? workdir;
}

// 运行状态徽标（票 05）：执行中绿点（reduced-motion 下不闪）、空闲黄点、已结束灰点。
const STATE_DOT: Record<SessionRunState, string> = {
  running: "bg-[var(--color-success)] motion-safe:animate-pulse",
  idle: "bg-[var(--color-warning)]",
  stopped: "bg-muted-foreground/50",
};

export function SessionGroups({ rows, pinned, stateOf, onResume }: {
  rows: ClientUsageSessionRow[];
  /** 正在执行区的会话（已排序，且是 rows 的子集）。 */
  pinned: ClientUsageSessionRow[];
  stateOf: (r: ClientUsageSessionRow) => SessionRunState;
  onResume: (r: ClientUsageSessionRow) => void;
}) {
  const { t } = useTranslation();
  const stateLabel: Record<SessionRunState, string> = {
    running: t("sessions.stateRunning", "执行中"),
    idle: t("sessions.stateIdle", "空闲"),
    stopped: t("sessions.stateStopped", "已结束"),
  };
  // 行内展开（spec S6，禁抽屉）：可同时展开多行。
  const [open, setOpen] = useState<Set<string>>(() => new Set());
  const toggle = (key: string) => setOpen((s) => {
    const n = new Set(s);
    if (!n.delete(key)) n.add(key);
    return n;
  });
  const groups = new Map<string, ClientUsageSessionRow[]>();
  for (const r of rows) {
    const g = groups.get(r.workdir);
    if (g) g.push(r);
    else groups.set(r.workdir, [r]);
  }
  const duration = (r: ClientUsageSessionRow) => {
    const d = Math.max(0, r.lastCallMs - r.firstCallMs);
    return d < 3600_000
      ? t("sessions.durMin", "{{n}} 分", { n: Math.round(d / 60_000) })
      : t("sessions.durHour", "{{n}} 小时", { n: (d / 3600_000).toFixed(1) });
  };
  const pinnedKeys = new Set(pinned.map((r) => `${r.clientCode}:${r.sessionId}`));
  return (
    <div data-testid="sessions-list">
      {pinned.length > 0 && (
        <div className="grid grid-cols-[230px_minmax(0,1fr)] border-t pt-4 pb-2.5" data-testid="sessions-pinned">
          <div className="sticky top-0 self-start pe-4">
            <b className="flex items-center gap-2 text-lg font-semibold">
              <i aria-hidden className={`inline-block size-2 rounded-full ${STATE_DOT.running}`} />
              {t("sessions.pinnedTitle", "正在执行")}
            </b>
            <div className="mt-2.5 text-sm text-muted-foreground">{t("sessions.count", "{{n}} 个会话", { n: pinned.length })}</div>
          </div>
          <div>{pinned.map((r) => renderRow(r, true))}</div>
        </div>
      )}
      {[...groups.entries()].map(([workdir, items]) => {
        const calls = items.reduce((a, r) => a + r.calls, 0);
        const unpriced = items.reduce((a, r) => a + r.unpricedCalls, 0);
        const cost = items.reduce((a, r) => a + r.cost, 0);
        const visible = items.filter((r) => !pinnedKeys.has(`${r.clientCode}:${r.sessionId}`));
        // 组内会话全部在上方执行中：不留空组。
        if (visible.length === 0) return null;
        return (
          <div key={workdir} className="grid grid-cols-[230px_minmax(0,1fr)] border-t pt-4 pb-2.5" data-testid="sessions-group" data-workdir={workdir} data-count={String(items.length)} data-cost={String(cost)}>
            <div className="sticky top-0 self-start pe-4">
              <b className="block truncate text-lg font-semibold" title={workdir}>{workdir ? projectName(workdir) : t("sessions.noWorkdir", "未知目录")}</b>
              {workdir && <small dir="ltr" className="mt-1 block truncate font-mono text-sm text-muted-foreground rtl:text-right" title={workdir} data-testid="sessions-group-path">{workdir}</small>}
              <div className="mt-2.5 text-sm text-muted-foreground">
                {t("sessions.count", "{{n}} 个会话", { n: items.length })} · <span className="font-mono tabular-nums"><SessionCost cost={cost} calls={calls} unpricedCalls={unpriced} /></span>
              </div>
            </div>
            <div>{visible.map((r) => renderRow(r, false))}</div>
          </div>
        );
      })}
    </div>
  );

  function renderRow(r: ClientUsageSessionRow, showDir: boolean) {
    const key = `${r.clientCode}:${r.sessionId}`;
    const isOpen = open.has(key);
    const runState = stateOf(r);
    const cache = t("sessions.cacheSplit", "缓存 读 {{read}} / 写 {{write}}", { read: formatNumber(r.cacheReadTokens), write: formatNumber(r.cacheWriteTokens) });
    return (
      <Fragment key={key}>
      <div
        role="button"
        tabIndex={0}
        aria-expanded={isOpen}
        aria-label={isOpen ? t("sessions.collapse", "收起详情") : t("sessions.expand", "展开详情")}
        onClick={() => toggle(key)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggle(key); }
        }}
        className={`${ROW_GRID} cursor-pointer rounded-lg px-3 py-2.5 hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring ${isOpen ? "bg-muted" : ""}`}
        data-testid="sessions-row"
        data-client={r.clientCode}
        data-session={r.sessionId}
        data-state={isOpen ? "open" : "closed"}
      >
        <ClientIcon code={r.clientCode} size={16} />
        <div className="min-w-0">
          <div className="truncate" title={r.title || r.sessionId}>{r.title || r.sessionId}</div>
          <div className="mt-0.5 truncate text-sm text-muted-foreground">
            <span className="inline-flex items-center gap-1.5 text-foreground" data-testid="sessions-row-state" data-state={runState}>
              <i aria-hidden className={`inline-block size-2 rounded-full ${STATE_DOT[runState]}`} />
              {stateLabel[runState]}
            </span>
            {" · "}
            {showDir && r.workdir && <><span dir="ltr" title={r.workdir}>{projectName(r.workdir)}</span>{" · "}</>}
            {clientName(r.clientCode)} · {t("sessions.calls", "{{n}} 次调用", { n: r.calls })} · {t("sessions.duration", "持续 {{d}}", { d: duration(r) })}
          </div>
        </div>
        <div className="text-end font-mono text-sm tabular-nums">
          {formatNumber(r.inputTokens + r.outputTokens)}
          <div className="truncate font-sans text-muted-foreground">
            {t("sessions.tokenSplit", "入 {{input}} / 出 {{output}}", { input: formatNumber(r.inputTokens), output: formatNumber(r.outputTokens) })}
          </div>
          <div className="truncate font-sans text-muted-foreground" title={cache} data-testid="sessions-row-cache" data-read={String(r.cacheReadTokens)} data-write={String(r.cacheWriteTokens)}>
            {cache}
          </div>
        </div>
        <div className="text-end font-mono text-sm tabular-nums">
          <SessionCost cost={r.cost} calls={r.calls} unpricedCalls={r.unpricedCalls} testId="sessions-row-cost" />
        </div>
        <div className="flex items-center justify-end gap-1 text-sm text-muted-foreground">
          {formatRelativeTime(r.lastCallMs)}
          <ChevronDown aria-hidden className={`size-4 shrink-0 transition-transform motion-reduce:transition-none ${isOpen ? "rotate-180" : ""}`} />
        </div>
        <div className="text-end" data-testid="sessions-row-actions">
          {/* Gemini 会话按项目目录隔离，无目录无法继续（票 07）；按钮阻冒泡，不展开行 */}
          {!(r.clientCode === "gemini_cli" && !r.workdir) && (
            <Button
              size="sm"
              variant="outline"
              data-testid="sessions-resume"
              onClick={(e) => { e.stopPropagation(); onResume(r); }}
              onKeyDown={(e) => e.stopPropagation()}
            >
              {t("sessions.resume", "继续")}
            </Button>
          )}
        </div>
      </div>
      {isOpen && <SessionDetail clientCode={r.clientCode} sessionId={r.sessionId} />}
      </Fragment>
    );
  }
}
