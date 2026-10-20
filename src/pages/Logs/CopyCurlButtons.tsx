// 票 passthrough-log-fixes/04：日志详情工具栏两按钮 —「复制用户请求 curl」「复制上游请求 curl」。
// 生成逻辑在 ./curl（buildCurl）；log.db 落库头已整值脱敏，{key} 占位换 key 即重放。
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { writeText } from "../../services/platform";
import { Button } from "@/components/ui/button";
import { safeParseJson } from "./types";
import { buildCurl } from "./curl";
import type { ProxyLogDetail } from "../../services/api";

type Side = "user" | "upstream";

export function CopyCurlButtons({ detail }: { detail: ProxyLogDetail }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState<Side | null>(null);
  // 卸载后不再 setState（快速关面板时 2s 复位定时器还挂着）
  const copiedTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(copiedTimer.current), []);

  const copy = async (side: Side) => {
    const isUser = side === "user";
    const curl = buildCurl({
      url: (isUser ? detail.request_url : detail.upstream_request_url) || "",
      headers: safeParseJson(
        (isUser ? detail.request_headers : detail.upstream_request_headers) || "{}",
      ),
      body: (isUser ? detail.request_body : detail.upstream_request_body) || undefined,
    });
    try {
      await writeText(curl);
      setCopied(side);
      clearTimeout(copiedTimer.current);
      copiedTimer.current = setTimeout(() => setCopied(null), 2000);
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <>
      {(["user", "upstream"] as const).map((side) => (
        <Button
          key={side}
          variant="ghost"
          className="btn-icon"
          onClick={() => copy(side)}
          title={t(side === "user" ? "logs.copyUserCurl" : "logs.copyUpstreamCurl", side === "user" ? "复制用户请求 curl" : "复制上游请求 curl")}
        >
          {copied === side ? (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="var(--color-success)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 7.5l3 3 7-7" /></svg>
          ) : (
            <svg width="16" height="16" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M5 2.5h6a1 1 0 011 1v6a1 1 0 01-1 1" /><rect x="2" y="4.5" width="8" height="7" rx="1" /></svg>
          )}
        </Button>
      ))}
    </>
  );
}
