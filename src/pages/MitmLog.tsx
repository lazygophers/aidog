// MITM 观测页（cc-sub-mitm 增量任务 3）：oauth 观测 + 旁路流量（mitm_log 表），
// 从「统计」页折叠面板提升为独立侧栏页。行点击展开内联详情（body 两列按需拉取，
// 开关关 / token 路径时天然空串——有值可查看，无值显占位文案）。数据打开页面拉一次，
// 无轮询。「日志」页（proxy_log）只留 AI 请求，观测行不混入。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { mitmStatsApi, type MitmBypassRow, type MitmBypassDetail } from "../services/api";
import { formatBytes, formatDateTime } from "../utils/formatters";
import { F } from "../domains/shared/tokens";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";

const prettyJson = (s: string) => {
  try {
    return JSON.stringify(JSON.parse(s), null, 2);
  } catch {
    return s;
  }
};

function BodyBlock({ label, body, emptyText }: { label: string; body: string; emptyText: string }) {
  return (
    <div style={{ minWidth: 0, flex: 1 }}>
      <div className="text-tertiary" style={{ fontSize: F.hint, fontWeight: 600, marginBottom: 2 }}>{label}</div>
      <pre style={{
        margin: 0, padding: 6, borderRadius: "var(--radius-sm)",
        background: "var(--bg-primary)", border: "1px solid var(--border)",
        fontSize: 11, fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
        whiteSpace: "pre-wrap", wordBreak: "break-all", maxHeight: 240, overflow: "auto",
      }}>
        {body ? prettyJson(body) : emptyText}
      </pre>
    </div>
  );
}

export function MitmLog() {
  const { t } = useTranslation();
  const [rows, setRows] = useState<MitmBypassRow[] | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);
  const [details, setDetails] = useState<Record<number, MitmBypassDetail | null>>({});

  useEffect(() => {
    let cancelled = false;
    mitmStatsApi.bypassList(200)
      .then(r => { if (!cancelled) setRows(r); })
      .catch(() => { if (!cancelled) setRows([]); });
    return () => { cancelled = true; };
  }, []);

  const toggle = (id: number) => {
    if (openId === id) {
      setOpenId(null);
      return;
    }
    setOpenId(id);
    if (!(id in details)) {
      mitmStatsApi.bypassDetail(id)
        .then(d => setDetails(prev => ({ ...prev, [id]: d ?? null })))
        .catch(() => setDetails(prev => ({ ...prev, [id]: null })));
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div className="section-title">{t("page.mitmLog", "MITM 观测")}</div>
      {rows === null ? (
        <div className="text-tertiary" style={{ fontSize: F.hint }}>{t("status.loading", "加载中…")}</div>
      ) : rows.length === 0 ? (
        <div className="text-tertiary" style={{ fontSize: F.hint }}>{t("stats.bypassEmpty", "（暂无旁路观测行）")}</div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("stats.bypassTime", "时间")}</TableHead>
              <TableHead>{t("stats.bypassGroup", "分组")}</TableHead>
              <TableHead>{t("stats.bypassHost", "Host / Path")}</TableHead>
              <TableHead style={{ textAlign: "right" }}>{t("stats.bypassStatus", "状态")}</TableHead>
              <TableHead style={{ textAlign: "right" }}>{t("stats.bypassBytes", "流量")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map(r => (
              <>
                <TableRow
                  key={r.id}
                  style={{ cursor: "pointer" }}
                  onClick={() => toggle(r.id)}
                  aria-expanded={openId === r.id}
                >
                  <TableCell style={{ padding: "6px 8px", whiteSpace: "nowrap" }}>{formatDateTime(r.created_at) ?? "-"}</TableCell>
                  <TableCell style={{ padding: "6px 8px" }}>{r.group_name || "-"}</TableCell>
                  <TableCell style={{ padding: "6px 8px", fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace", fontSize: 11 }}>
                    {openId === r.id ? "▾ " : "▸ "}{r.host}{r.path}
                  </TableCell>
                  <TableCell style={{ textAlign: "right", padding: "6px 8px", color: r.status_code >= 400 ? "var(--color-danger)" : undefined }}>{r.status_code}</TableCell>
                  <TableCell style={{ textAlign: "right", padding: "6px 8px" }}>{formatBytes(r.req_bytes + r.resp_bytes)}</TableCell>
                </TableRow>
                {openId === r.id && (
                  <TableRow key={`${r.id}-detail`}>
                    <TableCell colSpan={5} style={{ padding: "6px 8px" }}>
                      {!(r.id in details) ? (
                        <div className="text-tertiary" style={{ fontSize: F.hint }}>{t("status.loading", "加载中…")}</div>
                      ) : (
                        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                          <BodyBlock
                            label={t("page.mitmBodyRequest", "请求 body")}
                            body={details[r.id]?.request_body ?? ""}
                            emptyText={t("page.mitmBodyEmpty", "（未记录：日志开关关或该路径永不落库）")}
                          />
                          <BodyBlock
                            label={t("page.mitmBodyResponse", "响应 body")}
                            body={details[r.id]?.response_body ?? ""}
                            emptyText={t("page.mitmBodyEmpty", "（未记录：日志开关关或该路径永不落库）")}
                          />
                        </div>
                      )}
                    </TableCell>
                  </TableRow>
                )}
              </>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}

export default MitmLog;
