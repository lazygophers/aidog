// ─── 平台「分享」弹窗 ──────────────────────────
// 展示单平台可分享配置（含明文 api_key）→ 打开即自动复制剪贴板 + 手动复制按钮。
// 格式切换器 YAML(默认) / JSON / Base64 三路；视觉沿用 glass-elevated overlay 范式。
// 剪贴板走 Tauri 插件 writeText：macOS WKWebView 无手势激活时 navigator.clipboard 被拒静默失败，
// Tauri 侧走权限系统更可靠。
// Dialog 走 Radix Portal（替代 shared/Modal，liquid glass 居中由 Portal 保证）。

import { type CSSProperties, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { stringify as yamlStringify } from "yaml";
import QRCode from "qrcode";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

// ponytail: QR v40 L 级二进制上限 2953B，留余量 2900 触发降级提示
const QR_MAX_URL_LEN = 2900;

export type ShareFormat = "yaml" | "json" | "base64" | "url";

export interface ShareModalProps<T extends object = Record<string, unknown>> {
  /** 可分享配置对象（平台 SharePlatform / mcp {mcpServers:...} / 后续 skill 等）。 */
  share: T;
  /** 弹窗标题（平台名 / mcp 名 / ...）。 */
  title: string;
  /** 复制结果 toast：ok=true 成功，false 失败。 */
  onToast: (text: string, ok: boolean) => void;
  onClose: () => void;
  /** 标题文案 key（默认 platform.share.title）；mcp / skill 等可覆盖。 */
  titleKey?: string;
  /** 警示文案 key（默认 platform.share.warning）；mcp 等改用 mcp.share.warning。 */
  warningKey?: string;
  /** 提供 → 格式 tab 增 "url" 选项 + 渲染 QR：拼 `${urlScheme}?data=<base64>`。
   * base64 恒取 JSON 序列化（接收端 mcp_import_json 严格 JSON 解析），与展示格式解耦。
   * 仅 mcp 等需深链唤起（接收端走 mcp_import_json）的场景传。 */
  urlScheme?: string;
  /** 兼容旧调用点（url 格式 tab 文案 key，默认 platform.share.format.url）。 */
  copyUrlKey?: string;
}

const BASE_FORMATS: ShareFormat[] = ["yaml", "json", "base64"];

/** UTF-8 安全的 base64 编码（btoa 仅接 Latin-1，中文 / emoji 会抛错）。 */
export function toBase64Utf8(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

/** 按所选格式把分享对象转文本。base64 包 YAML（解析端 serde_yml 兼容）。
 * url 格式：拼 `${urlScheme}?data=<base64(JSON)>`（与 QR / 旧 copyUrl 同源）。 */
function formatShare<T extends object>(share: T, fmt: ShareFormat, urlScheme?: string): string {
  if (fmt === "url") {
    return `${urlScheme}?data=${toBase64Utf8(JSON.stringify(share, null, 2))}`;
  }
  const yaml = yamlStringify(share);
  if (fmt === "yaml") return yaml;
  if (fmt === "json") return JSON.stringify(share, null, 2);
  return toBase64Utf8(yaml);
}

export function ShareModal<T extends object = Record<string, unknown>>({
  share,
  title,
  onToast,
  onClose,
  titleKey,
  warningKey,
  urlScheme,
}: ShareModalProps<T>) {
  const { t } = useTranslation();
  // ponytail: url 格式优先排首位（深链是推荐分享格式），仅 urlScheme 存在时才有
  const formats: ShareFormat[] = urlScheme ? ["url", ...BASE_FORMATS] : BASE_FORMATS;
  const [format, setFormat] = useState<ShareFormat>(urlScheme ? "url" : "yaml");
  const [copied, setCopied] = useState(false);
  const copiedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const text = useMemo(() => formatShare(share, format, urlScheme), [share, format, urlScheme]);

  const deepLinkUrl = useMemo(() => {
    if (!urlScheme) return null;
    const url = `${urlScheme}?data=${toBase64Utf8(JSON.stringify(share, null, 2))}`;
    return url.length > QR_MAX_URL_LEN ? null : url;
  }, [urlScheme, share]);

  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!deepLinkUrl) {
      setQrDataUrl(null);
      return;
    }
    let cancelled = false;
    QRCode.toDataURL(deepLinkUrl, { errorCorrectionLevel: "L", margin: 1 })
      .then((d) => {
        if (!cancelled) setQrDataUrl(d);
      })
      .catch(() => {
        if (!cancelled) setQrDataUrl(null);
      });
    return () => {
      cancelled = true;
    };
  }, [deepLinkUrl]);

  const copy = async (auto: boolean) => {
    try {
      await writeText(text);
      onToast(
        auto
          ? t("platform.share.autoCopied", "分享内容已自动复制到剪贴板")
          : t("platform.share.copied", "已复制到剪贴板"),
        true,
      );
      setCopied(true);
      if (copiedTimer.current) clearTimeout(copiedTimer.current);
      copiedTimer.current = setTimeout(() => setCopied(false), 1500);
    } catch {
      onToast(t("platform.share.copyFail", "复制失败，请手动选择内容"), false);
    }
  };

  useEffect(() => {
    void copy(true);
    return () => {
      if (copiedTimer.current) clearTimeout(copiedTimer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const fmtTabStyle = (active: boolean): CSSProperties => ({
    fontSize: 12.5,
    fontWeight: 600,
    padding: "5px 14px",
    borderRadius: "var(--radius-sm)",
    cursor: "pointer",
    border: "1px solid",
    borderColor: active ? "var(--primary)" : "var(--border)",
    background: active ? "var(--accent-subtle)" : "transparent",
    color: active ? "var(--primary)" : "var(--text-secondary)",
  });

  return (
    <Dialog open onOpenChange={(next) => { if (!next) onClose(); }}>
      <DialogContent
        className="glass-elevated"
        style={{ maxWidth: 560, padding: "22px 24px", maxHeight: "86vh", overflowY: "auto" }}
      >
        <DialogHeader>
          <DialogTitle style={{ fontSize: 17 }}>
            {t(titleKey ?? "platform.share.title", "分享平台")} · {title}
          </DialogTitle>
        </DialogHeader>

        {/* 安全警示 */}
        <div
          style={{
            fontSize: 12.5,
            color: "var(--color-danger)",
            lineHeight: 1.55,
            padding: "8px 12px",
            borderRadius: "var(--radius-sm)",
            background: "var(--bg-glass)",
            border: "1px solid var(--color-danger)",
          }}
        >
          {t(
            warningKey ?? "platform.share.warning",
            "分享内容含明文 API Key，请仅发送给可信对象，切勿公开发布。",
          )}
        </div>

        {/* 格式切换器 */}
        <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
          {formats.map((f) => (
            <Button
              key={f}
              type="button"
              variant="ghost"
              style={fmtTabStyle(format === f)}
              onClick={() => setFormat(f)}
            >
              {t(`platform.share.format.${f}`, f.toUpperCase())}
            </Button>
          ))}
        </div>

        {/* 内容预览 */}
        <pre
          style={{
            flex: 1,
            minHeight: 160,
            maxHeight: "46vh",
            margin: 0,
            overflow: "auto",
            padding: "12px 14px",
            borderRadius: "var(--radius-sm)",
            background: "var(--bg-glass)",
            border: "1px solid var(--border)",
            fontFamily: "var(--font-mono, monospace)",
            fontSize: 12.5,
            lineHeight: 1.55,
            color: "var(--text-primary)",
            whiteSpace: "pre-wrap",
            wordBreak: "break-all",
          }}
        >
          {text}
        </pre>

        {/* QR 区块（仅 urlScheme 存在时显示；超长 / 生成失败 → 降级提示） */}
        {urlScheme && (
          <div
            style={{
              marginTop: 12,
              padding: "12px 14px",
              borderRadius: "var(--radius-sm)",
              background: "var(--bg-glass)",
              border: "1px solid var(--border)",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 8,
            }}
          >
            <div style={{ fontSize: 12.5, color: "var(--text-secondary)", fontWeight: 600 }}>
              {t("platform.share.scanToImport", "扫码导入")}
            </div>
            {qrDataUrl ? (
              <img
                src={qrDataUrl}
                alt={t("platform.share.scanToImport", "扫码导入")}
                style={{ width: 160, height: 160, borderRadius: "var(--radius-sm)", display: "block" }}
              />
            ) : (
              <div
                style={{
                  fontSize: 12,
                  color: "var(--text-secondary)",
                  textAlign: "center",
                  lineHeight: 1.55,
                  width: 200,
                }}
              >
                {t(
                  "platform.share.qrTooLong",
                  "内容过长，二维码不可用，请用复制链接",
                )}
              </div>
            )}
          </div>
        )}

        {/* 操作 */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 18, flexWrap: "wrap" }}>
          <Button variant="ghost" style={{ fontSize: 13, padding: "6px 14px" }} onClick={onClose}>
            {t("action.close", "关闭")}
          </Button>
          <Button
            style={{ fontSize: 13, padding: "6px 14px", minWidth: 96 }}
            onClick={() => void copy(false)}
          >
            {copied ? t("platform.share.copiedBtn", "已复制") : t("platform.share.copyBtn", "复制")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
