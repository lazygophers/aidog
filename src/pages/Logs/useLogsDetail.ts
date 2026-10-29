import { useState, useCallback, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { proxyLogApi, onProxyLogUpdated, type ProxyLogDetail } from "../../services/api";
import { writeText } from "../../services/platform";

/**
 * Logs 页详情态：当前打开的 ProxyLogDetail + 复制（全量 markdown / 单条 id）+ 打开/刷新详情。
 * 自 useLogsData 拆出，行为不变；openDetail/copyRow 同时被 ListView（触发打开）与
 * DetailPanel（详情内工具栏）依赖，是 list/detail 真实共享的一段，故独立成段而非塞进某一侧。
 */

  // 复制文本是给人读的调试文本，不进 UI，标签写死中文、不走 i18n
const REASON_LABELS: Record<string, string> = {
  "selected": "选中",
  "disabled-manual": "手动禁用",
  "disabled-auto": "自动禁用",
  "peak-disabled": "高峰期禁用",
  "breaker": "熔断",
  "quota-cooldown": "配额冷却",
  "auth-cooldown": "认证冷却",
  "unknown-protocol": "协议未知",
  "kind-unsupported": "请求类型不支持",
  "model-not-on-platform": "模型不在此平台",
  "skipped-dead-upstream": "死站跳过",
};

const TAG_LABELS: Record<string, string> = {
  "mapping-promoted": "映射提升",
  "coding-plan": "coding plan 上浮",
  "demoted": "失败降权",
  "sticky": "sticky",
};

/** 全量复制 Markdown；与详情页分区对应（快照/attempts 为空则整段省略，旧日志兼容）。 */
export function buildLogMarkdown(d: ProxyLogDetail): string {
  const fj = (s: string) => {
    try { return JSON.stringify(JSON.parse(s), null, 2); } catch { return s; }
  };
  const lines = [
      `# Proxy Log ${d.id}`,
      ``,
      `## Meta`,
      `- ID: ${d.id}`,
      `- Group: ${d.group_key}`,
      `- Model: ${d.model || "-"}`,
      `- Actual Model: ${d.actual_model || "-"}`,
      `- Source Protocol: ${d.source_protocol || "-"}`,
      `- Target Protocol: ${d.target_protocol || "-"}`,
      `- Status: ${d.status_code}`,
      `- Duration: ${d.duration_ms} ms`,
      `- Input Tokens: ${d.input_tokens}`,
      `- Output Tokens: ${d.output_tokens}`,
      `- Cache Tokens: ${d.cache_tokens}`,
      `- Time: ${d.created_at}`,
      ``,
    ];
    if (d.routing && d.routing.entries.length > 0) {
      lines.push(`## Routing Decision`);
      if (d.routing.mode) lines.push(`- Mode: ${d.routing.mode}`);
      lines.push(``);
      for (const c of d.routing.entries) {
        const tagText = c.tags.map(tg => TAG_LABELS[tg] ?? tg).join("、");
        const modelText = c.target_model ? ` · ${c.target_model}` : "";
        lines.push(`${c.order}. ${c.platform} — ${REASON_LABELS[c.reason] ?? c.reason}${tagText ? `（${tagText}）` : ""} · 优先级 L${c.level_priority}/P${c.priority}${modelText}`);
      }
      lines.push(``);
    }
    if (d.attempts.length > 0) {
      lines.push(`## Attempts`, ``);
      d.attempts.forEach((a, i) => {
        const modelText = a.model ? ` — ${a.model}` : "";
        lines.push(`${i + 1}. ${a.platform_name} — status ${a.status_code} — ${a.duration_ms} ms${modelText}${a.error ? ` — ${a.error}` : ""}`);
      });
      lines.push(``);
    }
    lines.push(
      `## User Request (Client → Proxy)`,
      `- URL: ${d.request_url || "-"}`,
      `- Status Code: ${d.status_code}`,
      `### Request Headers`,
      fj(d.request_headers),
      ``,
      `### Request Body`,
      fj(d.request_body),
      ``,
      `### Response Headers`,
      fj(d.user_response_headers || "{}"),
      ``,
      `### Response Body`,
      (d.user_response_body && d.user_response_body !== "[stream]")
        ? fj(d.user_response_body)
        : (d.response_body && d.response_body !== "[stream]")
          ? fj(d.response_body)
          : "(streaming, not captured)",
      ``,
      `## Upstream Request (Proxy → Platform)`,
      `- URL: ${d.upstream_request_url || "-"}`,
      `- Status Code: ${d.upstream_status_code || "-"}`,
      `### Request Headers`,
      fj(d.upstream_request_headers),
      ``,
      `### Request Body`,
      d.upstream_request_body ? fj(d.upstream_request_body) : "(not captured)",
      ``,
      `### Response Headers`,
      fj(d.upstream_response_headers || "{}"),
      ``,
      `### Response Body`,
      d.response_body ? fj(d.response_body) : "(streaming, not captured)",
    );
    return lines.join("\n");
  }
export function useLogsDetail() {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<ProxyLogDetail | null>(null);
  // 详情竞态守卫：当前请求的 id，迟到的旧响应据此丢弃（见 openDetail）
  const detailIdRef = useRef<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [copiedId, setCopiedId] = useState(false);


  const copyDetail = useCallback(async (d: ProxyLogDetail) => {
    try {
      await writeText(buildLogMarkdown(d));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) { console.error(e); }
  }, []);

  const openDetail = useCallback(async (id: string) => {
    // 竞态守卫：快速连点两行时慢的旧请求后到，不比对会覆盖面板显示错误请求的内容
    detailIdRef.current = id;
    try {
      const d = await proxyLogApi.get(id);
      if (d && detailIdRef.current === id) setDetail(d);
    } catch (e) { console.error(e); }
  }, []);

  const copyRow = useCallback(async (id: string) => {
    try {
      const d = await proxyLogApi.get(id);
      if (d) await copyDetail(d);
    } catch (err) { console.error(err); }
  }, [copyDetail]);

  const refreshDetail = useCallback(() => {
    if (!detail) return;
    proxyLogApi.get(detail.id)
      .then(d => { if (d) setDetail(d); })
      .catch(() => {});
  }, [detail]);
  useEffect(() => onProxyLogUpdated(() => { refreshDetail(); }, 1000), [refreshDetail]);

  return { t, detail, setDetail, copied, copiedId, setCopiedId, openDetail, copyDetail, copyRow };
}

export type LogsDetailData = ReturnType<typeof useLogsDetail>;
