import { useState, memo } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { F } from "../../domains/shared/tokens";
import type { ProxyLogSummary } from "../../services/api";
import type { TFunc } from "./types";

export const PAGE_SIZE_OPTIONS = [20, 50, 100] as const;

// ── 行内固定 style 提模块级常量（避免每行每次渲染重建对象，且让 LogRow memo 不被 inline 对象击穿）──
export const ROW_STYLE: React.CSSProperties = { cursor: "pointer", borderBottom: "1px solid var(--border)" };
export const INLINE_FLEX_STYLE: React.CSSProperties = { display: "inline-flex", alignItems: "center", gap: 6 };
export const PLATFORM_NAME_STYLE: React.CSSProperties = { fontSize: F.small, color: "var(--text-secondary)" };
export const RETRY_BADGE_STYLE: React.CSSProperties = { fontSize: 10, padding: "1px 5px", background: "color-mix(in srgb, var(--color-warning) 16%, transparent)", color: "var(--color-warning)" };
export const MODEL_NAME_STYLE: React.CSSProperties = { fontWeight: 500, fontSize: F.small };
export const SSE_BADGE_STYLE: React.CSSProperties = { fontSize: 10, padding: "1px 5px", background: "var(--accent-subtle)", color: "var(--accent, #007aff)" };
export const ACTION_BTN_STYLE: React.CSSProperties = { padding: 2 };
export const GROUP_BADGE_STYLE: React.CSSProperties = { fontSize: 11 };

// ponytail: CopyButton 自管 copied state（不挤占父级 copied/copiedId），复用已 import 的 writeText + 现有 copy/check svg 对
const COPY_ICON_STYLE: React.CSSProperties = {
  position: "absolute", top: 4, right: 4, zIndex: 2,
  display: "inline-flex", alignItems: "center", justifyContent: "center",
  width: 24, height: 24, padding: 0,
  background: "color-mix(in srgb, var(--bg-surface) 70%, transparent)",
  border: "1px solid var(--border)", borderRadius: 6, cursor: "pointer",
  color: "var(--text-secondary)", opacity: 0.55, transition: "opacity 0.15s ease",
};

export function CopyButton({ text, title }: { text: string; title?: string }) {
  const [copied, setCopied] = useState(false);
  if (!text) return null;
  return (
    <button
      type="button"
      className="copy-btn"
      style={COPY_ICON_STYLE}
      title={title}
      onClick={async (e) => {
        e.stopPropagation();
        try {
          await writeText(text);
          setCopied(true);
          setTimeout(() => setCopied(false), 2000);
        } catch (err) { console.error(err); }
      }}
    >
      {copied ? (
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="var(--color-success, var(--color-success))" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
      ) : (
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
      )}
    </button>
  );
}

export function MetaItem({ label, value, highlight, copyText, t }: { label: string; value: string; highlight?: "ok" | "err"; copyText?: string; t?: TFunc }) {
  return (
    <div style={{ position: "relative" }}>
      <div style={{ fontSize: F.small, color: "var(--text-tertiary)", marginBottom: 2 }}>{label}</div>
      <div style={{
        fontSize: F.body, fontWeight: 600,
        color: highlight === "ok" ? "var(--color-success, var(--color-success))" : highlight === "err" ? "var(--color-danger, #ff3b30)" : "var(--text-primary)",
        paddingRight: copyText ? 24 : undefined,
      }}>
        {value}
      </div>
      {copyText && copyText !== "-" && <CopyButton text={copyText} title={t?.("logs.copy", "复制")} />}
    </div>
  );
}

/** Tab 切换：用户请求 / 上游请求 */
export function RequestTabs({
  userTab, upstreamTab, t,
}: {
  userTab: { title: string; subtitle: string; protocol?: string; url?: string; statusCode?: number; reqHeaders: any; reqBody: any; respHeaders: any; respBody: any };
  upstreamTab: { title: string; subtitle: string; protocol?: string; url?: string; statusCode?: number; reqHeaders: any; reqBody: any; respHeaders: any; respBody: any };
  t: TFunc;
}) {
  const [active, setActive] = useState<"user" | "upstream">("user");
  const tab = active === "user" ? userTab : upstreamTab;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 0 }}>
      <div style={{ display: "flex", gap: 0, borderBottom: "1px solid var(--border)" }}>
        {(["user", "upstream"] as const).map((key) => {
          const item = key === "user" ? userTab : upstreamTab;
          const isActive = active === key;
          return (
            <button
              key={key}
              type="button"
              onClick={() => setActive(key)}
              style={{
                padding: "10px 20px", fontSize: F.hint, fontWeight: isActive ? 700 : 400,
                color: isActive ? "var(--accent)" : "var(--text-secondary)",
                background: "transparent", border: "none", cursor: "pointer",
                borderBottom: isActive ? "2px solid var(--accent)" : "2px solid transparent",
                transition: "all 0.15s ease",
                display: "flex", alignItems: "center", gap: 8,
              }}
            >
              {item.title}
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)", fontWeight: 400 }}>{item.subtitle}</span>
              {item.protocol && <span className="badge" style={{ fontSize: 10, padding: "1px 5px" }}>{item.protocol}</span>}
              {item.statusCode != null && item.statusCode > 0 && (
                <span style={{
                  fontSize: F.small, fontWeight: 600,
                  color: item.statusCode >= 200 && item.statusCode < 300 ? "var(--color-success, var(--color-success))" : "var(--color-danger, #ff3b30)",
                }}>
                  {item.statusCode}
                </span>
              )}
            </button>
          );
        })}
      </div>

      <div className="glass-surface" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 12 }}>
        <RequestSectionContent {...tab} t={t} />
      </div>
    </div>
  );
}

/** 请求内容渲染（无折叠，纯内容） */
function RequestSectionContent({
  url, reqHeaders, reqBody, respHeaders, respBody, t,
}: {
  url?: string;
  reqHeaders: any;
  reqBody: any;
  respHeaders: any;
  respBody: any;
  t: TFunc;
}) {
  const bodyStr = (v: any) => typeof v === "string" ? v : JSON.stringify(v, null, 2);
  const emptyBody = !reqBody && !respBody;
  // ponytail: 占位串（未捕获 / 流式未记录）不复制的判定基准 — 对比当前 locale 翻译后的占位文本
  const streamPlaceholder = t("logs.streamResponse", "(流式响应，内容未记录)");
  const isPlaceholder = (s: string) => !s || s === streamPlaceholder;
  const headersText = (h: any) => typeof h === "string" ? h : JSON.stringify(h, null, 2);
  const headersEmpty = (h: any) => !h || headersText(h) === "{}";
  const copyTitle = t("logs.copy", "复制");

  if (emptyBody) {
    return (
      <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontStyle: "italic" }}>
        {t("logs.noUpstream", "(未捕获)")}
      </div>
    );
  }

  return (
    <>
      {url && (
        <div>
          <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>URL</div>
          <div style={{ position: "relative" }}>
            <pre className="code-block" style={{ maxHeight: 60, overflow: "auto", wordBreak: "break-all", whiteSpace: "pre-wrap" }}>{url}</pre>
            <CopyButton text={url} title={copyTitle} />
          </div>
        </div>
      )}
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.requestHeaders", "请求头")}
        </div>
        <div style={{ position: "relative" }}>
          <pre className="code-block" style={{ maxHeight: 200, overflow: "auto" }}>{headersText(reqHeaders)}</pre>
          {!headersEmpty(reqHeaders) && <CopyButton text={headersText(reqHeaders)} title={copyTitle} />}
        </div>
      </div>
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.requestBody", "请求体")}
        </div>
        {reqBody
          ? (
            <div style={{ position: "relative" }}>
              <pre className="code-block" style={{ maxHeight: 300, overflow: "auto" }}>{bodyStr(reqBody)}</pre>
              {!isPlaceholder(bodyStr(reqBody)) && <CopyButton text={bodyStr(reqBody)} title={copyTitle} />}
            </div>
          )
          : <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontStyle: "italic" }}>-</div>
        }
      </div>
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.responseHeaders", "响应头")}
        </div>
        <div style={{ position: "relative" }}>
          <pre className="code-block" style={{ maxHeight: 200, overflow: "auto" }}>{headersText(respHeaders)}</pre>
          {!headersEmpty(respHeaders) && <CopyButton text={headersText(respHeaders)} title={copyTitle} />}
        </div>
      </div>
      <div>
        <div style={{ fontSize: F.small, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 4 }}>
          {t("logs.responseBody", "响应体")}
        </div>
        <div style={{ position: "relative" }}>
          <pre className="code-block" style={{ maxHeight: 400, overflow: "auto" }}>{bodyStr(respBody)}</pre>
          {!isPlaceholder(bodyStr(respBody)) && <CopyButton text={bodyStr(respBody)} title={copyTitle} />}
        </div>
      </div>
    </>
  );
}

export function ThCell({ children, sticky }: { children: React.ReactNode; sticky?: boolean }) {
  return (
    <th style={{
      padding: "10px 14px", textAlign: "left", fontWeight: 600,
      color: "var(--text-secondary)", whiteSpace: "nowrap", fontSize: F.small,
      ...(sticky ? {
        position: "sticky" as const, right: 0, zIndex: 2,
        background: "var(--bg-surface)",
        boxShadow: "-4px 0 8px -4px var(--shadow-color, rgba(0,0,0,0.08))",
      } : {}),
    }}>
      {children}
    </th>
  );
}

export function TdCell({ children, sticky }: { children: React.ReactNode; sticky?: boolean }) {
  return (
    <td style={{
      padding: "10px 14px", whiteSpace: "nowrap",
      ...(sticky ? {
        position: "sticky" as const, right: 0, zIndex: 2,
        background: "var(--bg-surface)",
        boxShadow: "-4px 0 8px -4px var(--shadow-color, rgba(0,0,0,0.08))",
      } : {}),
    }}>
      {children}
    </td>
  );
}

/**
 * 日志列表单行。React.memo + 稳定 props（platformName 传字符串、onOpen/onCopy 传 useCallback、style 用模块级常量）
 * → 父组件因轮询/筛选频繁重渲染时，未变化的行跳过重渲染、不重建行内 style 对象。
 */
interface LogRowProps {
  log: ProxyLogSummary;
  platformName: string;
  groupName: string;
  onOpen: (id: string) => void;
  onCopy: (id: string) => void;
  t: TFunc;
}

export const LogRow = memo(function LogRow({ log, platformName, groupName, onOpen, onCopy, t }: LogRowProps) {
  return (
    <tr
      className="log-row"
      onClick={() => onOpen(log.id)}
      style={ROW_STYLE}>
      <TdCell>{new Date(log.created_at).toLocaleString()}</TdCell>
      <TdCell><span className="badge badge-accent" style={GROUP_BADGE_STYLE}>{groupName}</span></TdCell>
      <TdCell>
        <span style={INLINE_FLEX_STYLE}>
          <span style={PLATFORM_NAME_STYLE}>{platformName}</span>
          {log.retry_count > 0 && (
            <span className="badge" style={RETRY_BADGE_STYLE}
              title={t("logs.retriedHint", "经过 {{n}} 次重试").replace("{{n}}", String(log.retry_count))}>
              ↻{log.retry_count}
            </span>
          )}
        </span>
      </TdCell>
      <TdCell>
        <span style={INLINE_FLEX_STYLE}>
          <span style={MODEL_NAME_STYLE}>{log.model || "-"}</span>
          {log.is_stream && (
            <span className="badge" style={SSE_BADGE_STYLE} title={t("logs.streaming", "流式")}>SSE</span>
          )}
        </span>
      </TdCell>
      <TdCell><span style={MODEL_NAME_STYLE}>{log.actual_model || "-"}</span></TdCell>
      <TdCell>
        <span style={{ color: log.status_code >= 200 && log.status_code < 300 ? "var(--color-success, var(--color-success))" : "var(--color-danger, #ff3b30)" }}>
          {log.status_code === 0
            ? t("logs.statusIncomplete", "未完成")
            : log.status_code === 499
              ? t("logs.statusInterrupted", "已中断")
              : log.status_code}
        </span>
      </TdCell>
      <TdCell>{log.duration_ms}ms</TdCell>
      <TdCell>{log.input_tokens || "-"}</TdCell>
      <TdCell>{log.output_tokens || "-"}</TdCell>
      <TdCell sticky>
        <button
          className="btn btn-ghost btn-icon"
          style={ACTION_BTN_STYLE}
          title={t("logs.copyAll", "复制完整信息")}
          onClick={(e) => { e.stopPropagation(); onCopy(log.id); }}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
        </button>
      </TdCell>
    </tr>
  );
});

/** 分页导航：首页/上一页/页码按钮/下一页/末页 + 总数 */
export function Pagination({
  currentPage, totalPages, total, pageSize, onPageChange, onPageSizeChange, t,
}: {
  currentPage: number;
  totalPages: number;
  total: number;
  pageSize: number;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: number) => void;
  t: TFunc;
}) {
  const rangeStart = (currentPage - 1) * pageSize + 1;
  const rangeEnd = Math.min(currentPage * pageSize, total);

  const pages: (number | "ellipsis")[] = [];
  if (totalPages <= 7) {
    for (let i = 1; i <= totalPages; i++) pages.push(i);
  } else {
    pages.push(1);
    if (currentPage > 3) pages.push("ellipsis");
    const start = Math.max(2, currentPage - 1);
    const end = Math.min(totalPages - 1, currentPage + 1);
    for (let i = start; i <= end; i++) pages.push(i);
    if (currentPage < totalPages - 2) pages.push("ellipsis");
    pages.push(totalPages);
  }

  const btnStyle: React.CSSProperties = {
    fontSize: 12, padding: "4px 8px", minWidth: 28, textAlign: "center",
  };

  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span className="text-tertiary" style={{ fontSize: 12 }}>
          {rangeStart}–{rangeEnd} / {total}
        </span>
        <label style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
          <span className="text-tertiary" style={{ fontSize: 12 }}>{t("logs.pageSize", "每页")}</span>
          <select
            aria-label={t("logs.pageSize", "每页")}
            value={pageSize}
            onChange={e => onPageSizeChange(Number(e.target.value))}
            style={{
              fontSize: F.small,
              padding: "4px 8px",
              borderRadius: 6,
              border: "1px solid var(--border)",
              background: "var(--bg-secondary, rgba(255,255,255,0.05))",
              color: "var(--text-primary)",
              cursor: "pointer",
            }}
          >
            {PAGE_SIZE_OPTIONS.map(size => (
              <option key={size} value={size}>{size}</option>
            ))}
          </select>
        </label>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
        <button className="btn btn-ghost" style={btnStyle} disabled={currentPage <= 1}
          onClick={() => onPageChange(1)} title="First">⟪</button>
        <button className="btn btn-ghost" style={btnStyle} disabled={currentPage <= 1}
          onClick={() => onPageChange(currentPage - 1)}>←</button>
        {pages.map((p, i) =>
          p === "ellipsis" ? (
            <span key={`e${i}`} className="text-tertiary" style={{ fontSize: 12, padding: "0 4px" }}>…</span>
          ) : (
            <button key={p} className={`btn ${p === currentPage ? "" : "btn-ghost"}`}
              style={{
                ...btnStyle,
                ...(p === currentPage ? { fontWeight: 700, color: "var(--accent)" } : {}),
              }}
              onClick={() => onPageChange(p)}>{p}</button>
          ),
        )}
        <button className="btn btn-ghost" style={btnStyle} disabled={currentPage >= totalPages}
          onClick={() => onPageChange(currentPage + 1)}>→</button>
        <button className="btn btn-ghost" style={btnStyle} disabled={currentPage >= totalPages}
          onClick={() => onPageChange(totalPages)} title="Last">⟫</button>
      </div>
    </div>
  );
}

/** 通用筛选下拉 */
export function FilterSelect({
  value,
  onChange,
  options,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
  placeholder: string;
}) {
  return (
    <select
      value={value}
      onChange={e => onChange(e.target.value)}
      style={{
        fontSize: F.small,
        padding: "4px 8px",
        borderRadius: 6,
        border: "1px solid var(--border)",
        background: "var(--bg-secondary, rgba(255,255,255,0.05))",
        color: "var(--text-primary)",
        cursor: "pointer",
        maxWidth: 140,
      }}
    >
      <option value="">{placeholder}</option>
      {options.map(o => (
        <option key={o.value} value={o.value}>{o.label}</option>
      ))}
    </select>
  );
}

