// 工具日志页（日志统计 → 工具日志）：网关代执行 / HTTP MCP 的每次工具调用。
// 列表按 工具 / 触发方 / 分组 筛选分页；行点击展开内联详情（各引擎条数 / 耗时 / 错误 + 结果正文，按需拉取）。
// 搜索词 / 结果正文受「记录用户请求原始信息」开关控制，关时为空串，详情显占位。保留天数独立于请求日志。
// 页头与「工具」页统一（section-title + 描述行，票 05/06）；样式走 Tailwind + ui 组件，禁 inline style。
import { Fragment, useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toolLogApi, type ToolLog as ToolLogRow, type ToolLogFilter } from "../services/api";
import { formatDateTime, formatDurationMs } from "../utils/formatters";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";

const PAGE_SIZE = 50;
const TOOLS = ["web_search", "web_fetch", "generate_image"] as const;
const TRIGGERS = ["gateway", "mcp"] as const;
/** 筛选/表格单元 select 与输入框的统一控件样式（G3：小尺寸 + card 底 + border） */
const CONTROL = "h-8 rounded-md border border-border bg-card px-2 text-sm";

interface EngineDetail { engine: string; count: number; latency_ms: number; error: string | null }

function parseEngines(s: string): EngineDetail[] {
  try {
    const v = JSON.parse(s);
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}

function Detail({ row }: { row: ToolLogRow }) {
  const { t } = useTranslation();
  const engines = parseEngines(row.engines);
  return (
    <div data-testid={`tool-log-detail-${row.id}`} className="flex flex-col gap-2">
      {row.platform && (
        <div className="text-sm text-secondary">{t("toolLog.platform", "被绕开的平台")}：{row.platform}</div>
      )}
      {row.error && (
        <div className="text-sm text-destructive">{row.error}</div>
      )}
      {engines.length > 0 && (
        <table className="text-sm">
          <thead>
            <tr>
              <th className="px-2 py-0.5 text-left font-normal">{t("toolLog.engine", "引擎")}</th>
              <th className="px-2 py-0.5 text-right font-normal">{t("toolLog.count", "条数")}</th>
              <th className="px-2 py-0.5 text-right font-normal">{t("toolLog.latency", "耗时")}</th>
              <th className="px-2 py-0.5 text-left font-normal">{t("toolLog.error", "错误")}</th>
            </tr>
          </thead>
          <tbody>
            {engines.map((e) => (
              <tr key={e.engine} data-testid={`tool-log-engine-${e.engine}`}>
                <td className="px-2 py-0.5">{e.engine}</td>
                <td className="px-2 py-0.5 text-right">{e.count}</td>
                <td className="px-2 py-0.5 text-right">{formatDurationMs(e.latency_ms)}</td>
                <td className={`px-2 py-0.5 ${e.error ? "text-destructive" : ""}`}>{e.error ?? ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <pre
        data-testid={`tool-log-result-${row.id}`}
        className="m-0 max-h-60 overflow-auto whitespace-pre-wrap break-all rounded-md border border-border bg-card p-1.5 font-mono text-sm"
      >
        {row.result || t("toolLog.resultEmpty", "（正文未记录：请开启「记录用户请求原始信息」）")}
      </pre>
    </div>
  );
}

export function ToolLog() {
  const { t } = useTranslation();
  const [tool, setTool] = useState("");
  const [trigger, setTrigger] = useState("");
  const [group, setGroup] = useState("");
  const [offset, setOffset] = useState(0);
  const [page, setPage] = useState<{ total: number; rows: ToolLogRow[] } | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);
  const [details, setDetails] = useState<Record<number, ToolLogRow | null>>({});
  const [days, setDays] = useState("");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    let cancelled = false;
    toolLogApi.retentionGet().then((d) => { if (!cancelled) setDays(String(d)); }).catch(() => {});
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const filter: ToolLogFilter = {} as ToolLogFilter;
    if (tool) filter.tool = tool;
    if (trigger) filter.trigger = trigger;
    if (group.trim()) filter.group_name = group.trim();
    toolLogApi.list(filter, PAGE_SIZE, offset)
      .then((p) => { if (!cancelled) setPage(p); })
      .catch(() => { if (!cancelled) setPage({ total: 0, rows: [] }); });
    return () => { cancelled = true; };
  }, [tool, trigger, group, offset]);

  const onFilter = (set: (v: string) => void) => (v: string) => { set(v); setOffset(0); setOpenId(null); };

  const toggle = useCallback((id: number) => {
    setOpenId((cur) => (cur === id ? null : id));
    if (!(id in details)) {
      toolLogApi.get(id)
        .then((d) => setDetails((prev) => ({ ...prev, [id]: d ?? null })))
        .catch(() => setDetails((prev) => ({ ...prev, [id]: null })));
    }
  }, [details]);

  const saveRetention = async () => {
    const n = Number.parseInt(days, 10);
    if (!Number.isFinite(n) || n < 0) return;
    await toolLogApi.retentionSet(n);
    setSaved(true);
  };

  const total = page?.total ?? 0;

  return (
    <div className="flex flex-col gap-3">
      <div>
        <div data-testid="tool-log-header" className="section-title">{t("page.toolLog", "工具日志")}</div>
        <div data-testid="tool-log-desc" className="mt-1 text-sm text-secondary">
          {t("toolLog.desc", "网关代执行与 HTTP MCP 的每次工具调用流水")}
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 text-sm">
        <select data-testid="tool-log-filter-tool" value={tool} className={CONTROL}
          onChange={(e) => onFilter(setTool)(e.target.value)}>
          <option value="">{t("toolLog.allTools", "全部工具")}</option>
          {TOOLS.map((x) => <option key={x} value={x}>{x}</option>)}
        </select>
        <select data-testid="tool-log-filter-trigger" value={trigger} className={CONTROL}
          onChange={(e) => onFilter(setTrigger)(e.target.value)}>
          <option value="">{t("toolLog.allTriggers", "全部触发方")}</option>
          {TRIGGERS.map((x) => <option key={x} value={x}>{t(`toolLog.trigger.${x}`, x)}</option>)}
        </select>
        <Input data-testid="tool-log-filter-group" value={group} className="h-8 w-40 text-sm"
          placeholder={t("toolLog.groupPlaceholder", "分组名")}
          onChange={(e) => onFilter(setGroup)(e.target.value)} />
        <span className="ms-auto text-secondary">{t("toolLog.retentionDays", "保留天数（0 = 永久）")}</span>
        <Input data-testid="tool-log-retention-input" type="number" min={0} value={days} className="h-8 w-20 text-sm"
          onChange={(e) => { setDays(e.target.value); setSaved(false); }} />
        <Button data-testid="tool-log-retention-save" size="sm" variant="outline" onClick={saveRetention}>
          {saved ? t("toolLog.saved", "已保存") : t("toolLog.save", "保存")}
        </Button>
      </div>

      {page === null ? (
        <div className="text-sm text-tertiary">{t("status.loading", "加载中…")}</div>
      ) : page.rows.length === 0 ? (
        <div data-testid="tool-log-empty" className="text-sm text-tertiary">
          {t("toolLog.empty", "（暂无工具调用记录）")}
        </div>
      ) : (
        <>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("toolLog.time", "时间")}</TableHead>
                <TableHead>{t("toolLog.tool", "工具")}</TableHead>
                <TableHead>{t("toolLog.triggerCol", "触发方")}</TableHead>
                <TableHead>{t("toolLog.group", "分组")}</TableHead>
                <TableHead>{t("toolLog.query", "搜索词 / URL")}</TableHead>
                <TableHead className="text-right">{t("toolLog.count", "条数")}</TableHead>
                <TableHead className="text-right">{t("toolLog.duration", "总耗时")}</TableHead>
                <TableHead className="text-right">{t("toolLog.status", "状态")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {page.rows.map((r) => (
                <Fragment key={r.id}>
                  <TableRow data-testid={`tool-log-row-${r.id}`} className="cursor-pointer"
                    onClick={() => toggle(r.id)} aria-expanded={openId === r.id}>
                    <TableCell className="whitespace-nowrap">{formatDateTime(r.created_at) ?? "-"}</TableCell>
                    <TableCell className="font-mono text-sm">{openId === r.id ? "▾ " : "▸ "}{r.tool}</TableCell>
                    <TableCell>{t(`toolLog.trigger.${r.trigger}`, r.trigger)}</TableCell>
                    <TableCell>{r.group_name || "-"}</TableCell>
                    <TableCell className="max-w-[280px] truncate">{r.query || "-"}</TableCell>
                    <TableCell className="text-right">{r.result_count}</TableCell>
                    <TableCell className="text-right">{formatDurationMs(r.duration_ms)}</TableCell>
                    <TableCell className={`text-right ${r.success ? "" : "text-destructive"}`}>
                      {r.success ? t("toolLog.ok", "成功") : t("toolLog.fail", "失败")}
                    </TableCell>
                  </TableRow>
                  {openId === r.id && (
                    <TableRow>
                      <TableCell colSpan={8}>
                        {details[r.id] === undefined ? (
                          <div className="text-sm text-tertiary">{t("status.loading", "加载中…")}</div>
                        ) : (
                          <Detail row={details[r.id] ?? r} />
                        )}
                      </TableCell>
                    </TableRow>
                  )}
                </Fragment>
              ))}
            </TableBody>
          </Table>
          <div className="flex items-center gap-2 text-sm">
            <Button data-testid="tool-log-prev" size="sm" variant="outline" disabled={offset === 0}
              onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>{t("toolLog.prev", "上一页")}</Button>
            <span className="text-secondary">{offset + 1}–{Math.min(offset + PAGE_SIZE, total)} / {total}</span>
            <Button data-testid="tool-log-next" size="sm" variant="outline" disabled={offset + PAGE_SIZE >= total}
              onClick={() => setOffset(offset + PAGE_SIZE)}>{t("toolLog.next", "下一页")}</Button>
          </div>
        </>
      )}
    </div>
  );
}

export default ToolLog;
