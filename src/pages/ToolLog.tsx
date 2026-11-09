// 工具日志页（日志统计 → 工具日志）：网关代执行 / HTTP MCP 的每次工具调用。
// 列表按 工具 / 触发方 / 分组 筛选分页；行点击展开内联详情（各引擎条数 / 耗时 / 错误 + 结果正文，按需拉取）。
// 搜索词 / 结果正文受「记录用户请求原始信息」开关控制，关时为空串，详情显占位。保留天数独立于请求日志。
import { Fragment, useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toolLogApi, type ToolLog as ToolLogRow, type ToolLogFilter } from "../services/api";
import { formatDateTime, formatDurationMs } from "../utils/formatters";
import { F } from "../domains/shared/tokens";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";

const PAGE_SIZE = 50;
const TOOLS = ["web_search", "web_fetch", "generate_image", "generate_video"] as const;
const TRIGGERS = ["gateway", "mcp"] as const;
const MONO = "ui-monospace, SFMono-Regular, Menlo, monospace";

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
    <div data-testid={`tool-log-detail-${row.id}`} style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      {row.platform && (
        <div style={{ fontSize: F.hint }}>{t("toolLog.platform", "被绕开的平台")}：{row.platform}</div>
      )}
      {row.error && (
        <div style={{ fontSize: F.hint, color: "var(--color-danger)" }}>{row.error}</div>
      )}
      {engines.length > 0 && (
        <table style={{ fontSize: 11, borderCollapse: "collapse" }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", padding: "2px 8px" }}>{t("toolLog.engine", "引擎")}</th>
              <th style={{ textAlign: "right", padding: "2px 8px" }}>{t("toolLog.count", "条数")}</th>
              <th style={{ textAlign: "right", padding: "2px 8px" }}>{t("toolLog.latency", "耗时")}</th>
              <th style={{ textAlign: "left", padding: "2px 8px" }}>{t("toolLog.error", "错误")}</th>
            </tr>
          </thead>
          <tbody>
            {engines.map((e) => (
              <tr key={e.engine} data-testid={`tool-log-engine-${e.engine}`}>
                <td style={{ padding: "2px 8px" }}>{e.engine}</td>
                <td style={{ textAlign: "right", padding: "2px 8px" }}>{e.count}</td>
                <td style={{ textAlign: "right", padding: "2px 8px" }}>{formatDurationMs(e.latency_ms)}</td>
                <td style={{ padding: "2px 8px", color: e.error ? "var(--color-danger)" : undefined }}>{e.error ?? ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <pre
        data-testid={`tool-log-result-${row.id}`}
        style={{
          margin: 0, padding: 6, borderRadius: "var(--radius-sm)",
          background: "var(--bg-primary)", border: "1px solid var(--border)",
          fontSize: 11, fontFamily: MONO, whiteSpace: "pre-wrap", wordBreak: "break-all", maxHeight: 240, overflow: "auto",
        }}
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

  const selectStyle = { fontSize: 12, padding: "4px 6px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)", background: "var(--bg-primary)" };
  const total = page?.total ?? 0;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div className="section-title">{t("page.toolLog", "工具日志")}</div>

      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center", fontSize: 12 }}>
        <select data-testid="tool-log-filter-tool" value={tool} style={selectStyle}
          onChange={(e) => onFilter(setTool)(e.target.value)}>
          <option value="">{t("toolLog.allTools", "全部工具")}</option>
          {TOOLS.map((x) => <option key={x} value={x}>{x}</option>)}
        </select>
        <select data-testid="tool-log-filter-trigger" value={trigger} style={selectStyle}
          onChange={(e) => onFilter(setTrigger)(e.target.value)}>
          <option value="">{t("toolLog.allTriggers", "全部触发方")}</option>
          {TRIGGERS.map((x) => <option key={x} value={x}>{t(`toolLog.trigger.${x}`, x)}</option>)}
        </select>
        <Input data-testid="tool-log-filter-group" value={group} className="h-8 w-40 text-xs"
          placeholder={t("toolLog.groupPlaceholder", "分组名")}
          onChange={(e) => onFilter(setGroup)(e.target.value)} />
        <span style={{ marginInlineStart: "auto" }} className="text-secondary">{t("toolLog.retentionDays", "保留天数（0 = 永久）")}</span>
        <Input data-testid="tool-log-retention-input" type="number" min={0} value={days} className="h-8 w-20 text-xs"
          onChange={(e) => { setDays(e.target.value); setSaved(false); }} />
        <Button data-testid="tool-log-retention-save" size="sm" variant="outline" onClick={saveRetention}>
          {saved ? t("toolLog.saved", "已保存") : t("toolLog.save", "保存")}
        </Button>
      </div>

      {page === null ? (
        <div className="text-tertiary" style={{ fontSize: F.hint }}>{t("status.loading", "加载中…")}</div>
      ) : page.rows.length === 0 ? (
        <div data-testid="tool-log-empty" className="text-tertiary" style={{ fontSize: F.hint }}>
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
                <TableHead style={{ textAlign: "right" }}>{t("toolLog.count", "条数")}</TableHead>
                <TableHead style={{ textAlign: "right" }}>{t("toolLog.duration", "总耗时")}</TableHead>
                <TableHead style={{ textAlign: "right" }}>{t("toolLog.status", "状态")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {page.rows.map((r) => (
                <Fragment key={r.id}>
                  <TableRow data-testid={`tool-log-row-${r.id}`} style={{ cursor: "pointer" }}
                    onClick={() => toggle(r.id)} aria-expanded={openId === r.id}>
                    <TableCell style={{ padding: "6px 8px", whiteSpace: "nowrap" }}>{formatDateTime(r.created_at) ?? "-"}</TableCell>
                    <TableCell style={{ padding: "6px 8px", fontFamily: MONO, fontSize: 11 }}>{openId === r.id ? "▾ " : "▸ "}{r.tool}</TableCell>
                    <TableCell style={{ padding: "6px 8px" }}>{t(`toolLog.trigger.${r.trigger}`, r.trigger)}</TableCell>
                    <TableCell style={{ padding: "6px 8px" }}>{r.group_name || "-"}</TableCell>
                    <TableCell style={{ padding: "6px 8px", maxWidth: 280, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{r.query || "-"}</TableCell>
                    <TableCell style={{ textAlign: "right", padding: "6px 8px" }}>{r.result_count}</TableCell>
                    <TableCell style={{ textAlign: "right", padding: "6px 8px" }}>{formatDurationMs(r.duration_ms)}</TableCell>
                    <TableCell style={{ textAlign: "right", padding: "6px 8px", color: r.success ? undefined : "var(--color-danger)" }}>
                      {r.success ? t("toolLog.ok", "成功") : t("toolLog.fail", "失败")}
                    </TableCell>
                  </TableRow>
                  {openId === r.id && (
                    <TableRow>
                      <TableCell colSpan={8} style={{ padding: "6px 8px" }}>
                        {details[r.id] === undefined ? (
                          <div className="text-tertiary" style={{ fontSize: F.hint }}>{t("status.loading", "加载中…")}</div>
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
          <div style={{ display: "flex", gap: 8, alignItems: "center", fontSize: 12 }}>
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
