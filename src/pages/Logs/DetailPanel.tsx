import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { F } from "../../domains/shared/tokens";
import { CopyButton, MetaItem, RequestTabs } from "./primitives";
import { safeParseJson } from "./types";
import type { LogsData } from "./useLogsData";
import { formatDateTime } from "../../utils/formatters";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "@/components/ui/sheet";

/**
 * 日志详情视图（自原 Logs.tsx L265-453 外迁）。
 * 以 shadcn Sheet（Radix Portal 侧抽屉）叠加在列表之上；open = detail 非空。
 * 接 hook 提供的 detail/copy/openDetail 及 platform/group 映射，业务逻辑零改。
 */
export function DetailPanel({ d }: { d: LogsData }) {
  const { detail, t, copied, copiedId, setCopiedId, openDetail, copyDetail, platformMap, groupName, setDetail } = d;

  return (
    <Sheet open={detail !== null} onOpenChange={(o) => { if (!o) setDetail(null); }}>
      <SheetContent
        side="right"
        className="glass-elevated"
        // ponytail: 详情面板内容多（meta grid + attempts + 请求 tabs），需超宽 + 内部纵向滚动；
        // 覆盖 shadcn 默认 sm:max-w-sm 与 p-6。
        style={{
          width: "min(900px, 90vw)",
          maxWidth: "min(900px, 90vw)",
          padding: 20,
          gap: 16,
          overflowY: "auto",
          display: "flex",
          flexDirection: "column",
        }}
      >
        <SheetHeader>
          <SheetTitle className="sr-only">{t("logs.detail", "请求详情")}</SheetTitle>
          <SheetDescription className="sr-only">{detail?.id ?? ""}</SheetDescription>
        </SheetHeader>

        {!detail ? null : <DetailBody
          detail={detail}
          t={t}
          copied={copied}
          copiedId={copiedId}
          setCopiedId={setCopiedId}
          openDetail={openDetail}
          copyDetail={copyDetail}
          platformMap={platformMap}
          groupName={groupName}
        />}
      </SheetContent>
    </Sheet>
  );
}

interface DetailBodyProps {
  detail: NonNullable<LogsData["detail"]>;
  t: LogsData["t"];
  copied: boolean;
  copiedId: boolean;
  setCopiedId: (v: boolean) => void;
  openDetail: (id: string) => void;
  copyDetail: (d: NonNullable<LogsData["detail"]>) => void;
  platformMap: LogsData["platformMap"];
  groupName: (k: string) => string;
}

function DetailBody({ detail, t, copied, copiedId, setCopiedId, openDetail, copyDetail, platformMap, groupName }: DetailBodyProps) {
  const reqHeaders = safeParseJson(detail.request_headers);
  const reqBody = safeParseJson(detail.request_body);
  const upstreamHeaders = safeParseJson(detail.upstream_request_headers);
  const upstreamBody = detail.upstream_request_body
    ? safeParseJson(detail.upstream_request_body)
    : null;
  const upstreamRespHeaders = safeParseJson(detail.upstream_response_headers || "{}");
  // 流式日志现已聚合真实 SSE 内容；仅当仍为 "[stream]" 占位（日志开关关闭 / 内容未捕获）才显示提示。
  const upstreamRespBody = detail.response_body === "[stream]" || !detail.response_body
    ? t("logs.streamResponse", "(流式响应，内容未记录)")
    : safeParseJson(detail.response_body);
  const userRespHeaders = safeParseJson(detail.user_response_headers || "{}");
  const userRespBody = detail.user_response_body && detail.user_response_body !== "[stream]"
    ? safeParseJson(detail.user_response_body)
    : detail.response_body && detail.response_body !== "[stream]"
      ? safeParseJson(detail.response_body)
      : t("logs.streamResponse", "(流式响应，内容未记录)");

  return (
    <>
      {/* Header（自定义工具栏：刷新/复制；返回由 Sheet 右上角 X 关闭承担） */}
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <Button variant="ghost" className="btn-icon" onClick={() => openDetail(detail.id)} title={t("logs.refresh", "刷新")}>
          <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" /></svg>
        </Button>
        <Button variant="ghost" className="btn-icon" onClick={() => copyDetail(detail)} title={t("logs.copyAll", "复制完整信息")}>
          {copied ? (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="var(--color-success, var(--color-success))" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
          ) : (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
          )}
        </Button>
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: F.title, fontWeight: 700 }}>{t("logs.detail", "请求详情")}</div>
        </div>
      </div>

      {/* Request ID — full row */}
      <div className="glass-surface" style={{ padding: "12px 20", display: "flex", alignItems: "center", gap: 10 }}>
        <span style={{ fontSize: F.small, color: "var(--text-tertiary)", fontWeight: 600 }}>{t("logs.requestId", "请求 ID")}</span>
        <span style={{ fontSize: F.hint, fontFamily: "monospace", color: "var(--text-primary)" }}>{detail.id}</span>
        <Button
          variant="ghost"
          className="btn-icon"
          style={{ marginLeft: "auto" }}
          onClick={async () => {
            await writeText(`request_id=${detail.id}`);
            setCopiedId(true);
            setTimeout(() => setCopiedId(false), 2000);
          }}
          title={t("logs.copyRequestId", "复制请求 ID")}
        >
          {copiedId ? (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="var(--color-success, var(--color-success))" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
          ) : (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="8" height="8" rx="1" /><path d="M10 10v1.5a1 1 0 01-1 1h-6a1 1 0 01-1-1v-6a1 1 0 011-1H4.5" /></svg>
          )}
        </Button>
      </div>

      {/* Meta grid */}
      <div className="glass-surface" style={{ padding: 20, display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))", gap: 14 }}>
        <MetaItem label={t("logs.group", "分组")} value={groupName(detail.group_key)} copyText={detail.group_key} t={t} />
        <MetaItem label={t("logs.platform", "平台")} value={platformMap.get(detail.platform_id) || "-"} copyText={platformMap.get(detail.platform_id)} t={t} />
        <MetaItem label={t("logs.model", "原始模型")} value={detail.model || "-"} copyText={detail.model} t={t} />
        <MetaItem label={t("logs.actualModel", "实际模型")} value={detail.actual_model && detail.actual_model !== detail.model ? detail.actual_model : "-"} copyText={detail.actual_model && detail.actual_model !== detail.model ? detail.actual_model : undefined} t={t} />
        <MetaItem label={t("logs.sourceProtocol", "用户格式")} value={detail.source_protocol || "-"} copyText={detail.source_protocol} t={t} />
        <MetaItem label={t("logs.targetProtocol", "请求格式")} value={detail.target_protocol || "-"} copyText={detail.target_protocol} t={t} />
        <MetaItem
          label={t("logs.status", "状态")}
          value={
            detail.status_code === 0
              ? t("logs.statusIncomplete", "未完成")
              : detail.status_code === 499
                ? t("logs.statusInterrupted", "已中断")
                : `${detail.status_code}`
          }
          highlight={detail.status_code >= 200 && detail.status_code < 300 ? "ok" : "err"}
          copyText={`${detail.status_code}`}
          t={t}
        />
        <MetaItem
          label={t("logs.upstreamStatus", "上游状态")}
          value={
            detail.upstream_status_code === 0 || detail.upstream_status_code == null
              ? t("logs.notCaptured", "未捕获")
              : `${detail.upstream_status_code}`
          }
          highlight={
            detail.upstream_status_code === 0 || detail.upstream_status_code == null
              ? undefined
              : detail.upstream_status_code >= 200 && detail.upstream_status_code < 300 ? "ok" : "err"
          }
          copyText={
            detail.upstream_status_code === 0 || detail.upstream_status_code == null
              ? undefined
              : `${detail.upstream_status_code}`
          }
          t={t}
        />
        <MetaItem label={t("logs.stream", "传输")} value={detail.is_stream ? t("logs.streaming", "流式") : t("logs.nonStreaming", "非流式")} copyText={detail.is_stream ? t("logs.streaming", "流式") : t("logs.nonStreaming", "非流式")} t={t} />
        <MetaItem label={t("logs.duration", "耗时")} value={`${detail.duration_ms} ms`} copyText={`${detail.duration_ms} ms`} t={t} />
        <MetaItem label={t("logs.inputTokens", "输入 Token")} value={`${detail.input_tokens}`} copyText={`${detail.input_tokens}`} t={t} />
        <MetaItem label={t("logs.outputTokens", "输出 Token")} value={`${detail.output_tokens}`} copyText={`${detail.output_tokens}`} t={t} />
        <MetaItem label={t("logs.cacheTokens", "缓存 Token")} value={`${detail.cache_tokens}`} copyText={`${detail.cache_tokens}`} t={t} />
        <MetaItem label={t("logs.time", "时间")} value={formatDateTime(detail.created_at) || "-"} copyText={formatDateTime(detail.created_at) || "-"} t={t} />
      </div>

      {/* ── 平台尝试时序（多平台重试时展示每次尝试：平台/状态码/耗时/错误）── */}
      {detail.attempts && detail.attempts.length >= 1 && (
        <div className="glass-surface" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 10 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ fontSize: F.body, fontWeight: 600 }}>{t("logs.attempts", "尝试记录")}</span>
            <span className="badge" style={{ fontSize: 10, padding: "1px 6px", background: "color-mix(in srgb, var(--color-warning) 16%, transparent)", color: "var(--color-warning)" }}>
              {t("logs.attemptCount", "{{n}} 次").replace("{{n}}", String(detail.attempts.length))}
            </span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {detail.attempts.map((a, i) => {
              const ok = a.status_code >= 200 && a.status_code < 300;
              const platName = a.platform_name || platformMap.get(a.platform_id) || `#${a.platform_id}`;
              // 摘要串：平台名 | 状态码 | 耗时ms | 错误(若有)
              const summary = [platName, String(a.status_code), `${a.duration_ms}ms`, a.error].filter(Boolean).join(" | ");
              return (
                <div key={i} style={{
                  position: "relative",
                  display: "grid", gridTemplateColumns: "24px 1fr auto auto", alignItems: "center", gap: 10,
                  padding: "6px 28px 6px 10px", borderRadius: 8,
                  background: ok ? "color-mix(in srgb, var(--color-success) 8%, transparent)" : "color-mix(in srgb, var(--color-danger) 8%, transparent)",
                  border: `1px solid ${ok ? "color-mix(in srgb, var(--color-success) 25%, transparent)" : "color-mix(in srgb, var(--color-danger) 25%, transparent)"}`,
                }}>
                  <span style={{ fontSize: 11, color: "var(--text-tertiary)", fontFamily: "monospace" }}>#{i + 1}</span>
                  <span style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
                    <span style={{ fontSize: F.small, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {platName}
                    </span>
                    {a.error && (
                      <span style={{ fontSize: 10, color: "var(--color-danger)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={a.error}>
                        {a.error}
                      </span>
                    )}
                  </span>
                  <span style={{ fontSize: F.small, fontWeight: 600, color: ok ? "var(--color-success)" : "var(--color-danger)" }}>
                    {a.status_code === 0 ? t("logs.connFailed", "连接失败") : a.status_code}
                  </span>
                  <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>{a.duration_ms}ms</span>
                  <CopyButton text={summary} title={t("logs.copy", "复制")} />
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* ── 用户请求 / 上游请求 Tab ── */}
      <RequestTabs
        userTab={{
          title: t("logs.userRequest", "用户请求"),
          subtitle: "Client → Proxy",
          protocol: detail.source_protocol?.toUpperCase(),
          url: detail.request_url,
          statusCode: detail.status_code,
          reqHeaders,
          reqBody,
          respHeaders: userRespHeaders,
          respBody: userRespBody,
        }}
        upstreamTab={{
          title: t("logs.upstreamRequest", "上游请求"),
          subtitle: "Proxy → Platform",
          protocol: detail.target_protocol?.toUpperCase(),
          url: detail.upstream_request_url,
          statusCode: detail.upstream_status_code,
          reqHeaders: upstreamHeaders,
          reqBody: upstreamBody,
          respHeaders: upstreamRespHeaders,
          respBody: upstreamRespBody,
        }}
        t={t}
      />
    </>
  );
}
