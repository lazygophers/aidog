import { useState, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { proxyLogApi, onProxyLogUpdated, type ProxyLogDetail } from "../../services/api";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

/**
 * Logs 页详情态：当前打开的 ProxyLogDetail + 复制（全量 markdown / 单条 id）+ 打开/刷新详情。
 * 自 useLogsData 拆出，行为不变；openDetail/copyRow 同时被 ListView（触发打开）与
 * DetailPanel（详情内工具栏）依赖，是 list/detail 真实共享的一段，故独立成段而非塞进某一侧。
 */
export function useLogsDetail() {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<ProxyLogDetail | null>(null);
  const [copied, setCopied] = useState(false);
  const [copiedId, setCopiedId] = useState(false);

  const copyDetail = useCallback(async (d: ProxyLogDetail) => {
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
    ];
    try {
      await writeText(lines.join("\n"));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) { console.error(e); }
  }, []);

  const openDetail = useCallback(async (id: string) => {
    try {
      const d = await proxyLogApi.get(id);
      if (d) setDetail(d);
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
