// ─── 平台「分享」弹窗 ──────────────────────────
// 展示单平台可分享配置（含明文 api_key）→ 打开即自动复制剪贴板 + 手动复制按钮。
// 格式切换器 YAML(默认) / JSON / Base64 三路；视觉沿用 glass-elevated overlay 范式
// (参 UpdatePromptModal / SmartPasteModal)。剪贴板走 Tauri 插件 writeText：
// macOS WKWebView 无手势激活时 navigator.clipboard 被拒静默失败，Tauri 侧走权限系统更可靠。

import { type CSSProperties, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { stringify as yamlStringify } from "yaml";

export type ShareFormat = "yaml" | "json" | "base64";

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
  /** 提供 → 额外渲染「复制为 aidog:// URL」按钮：拼 `${urlScheme}?data=<base64>`。
   * base64 恒取 JSON 序列化（接收端 mcp_import_json 严格 JSON 解析），与展示格式解耦。
   * 仅 mcp 等需深链唤起（接收端走 mcp_import_json）的场景传。 */
  urlScheme?: string;
  /** 「复制 URL」按钮文案 key（默认不显示按钮）。 */
  copyUrlKey?: string;
}

const FORMATS: ShareFormat[] = ["yaml", "json", "base64"];

/** UTF-8 安全的 base64 编码（btoa 仅接 Latin-1，中文 / emoji 会抛错）。 */
export function toBase64Utf8(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

/** 按所选格式把分享对象转文本。base64 包裹 YAML（解析端 serde_yml 兼容）。 */
function formatShare<T extends object>(share: T, fmt: ShareFormat): string {
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
  copyUrlKey,
}: ShareModalProps<T>) {
  const { t } = useTranslation();
  const [format, setFormat] = useState<ShareFormat>("yaml");
  const [copied, setCopied] = useState(false);
  const copiedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const text = useMemo(() => formatShare(share, format), [share, format]);

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

  // 拼 aidog:// URL（mcp/skill 深链唤起用）。
  // base64 恒取 JSON 序列化（接收端 mcp_import_json 走严格 JSON 解析；YAML 非严格 JSON 会被拒），
  // 与弹窗当前展示格式解耦——URL 承载「可导入契约」，展示文本随用户切格式。
  const copyUrl = async () => {
    if (!urlScheme) return;
    try {
      const url = `${urlScheme}?data=${toBase64Utf8(JSON.stringify(share, null, 2))}`;
      await writeText(url);
      onToast(t("mcp.share.urlCopied", "已复制 aidog:// 链接"), true);
    } catch {
      onToast(t("platform.share.copyFail", "复制失败，请手动选择内容"), false);
    }
  };

  // 打开即自动复制当前格式（YAML 默认）。仅首次 mount 触发，切换格式不重复自动复制。
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
    borderColor: active ? "var(--accent)" : "var(--border)",
    background: active ? "var(--accent-subtle)" : "transparent",
    color: active ? "var(--accent)" : "var(--text-secondary)",
  });

  return createPortal(
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1100,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "rgba(0,0,0,0.5)",
        animation: "fadeIn 150ms ease both",
      }}
      onClick={onClose}
    >
      <div
        className="glass-elevated"
        style={{
          width: 560,
          maxWidth: "92vw",
          maxHeight: "86vh",
          display: "flex",
          flexDirection: "column",
          borderRadius: "var(--radius-lg)",
          animation: "fadeIn 200ms ease both",
          padding: "22px 24px",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ fontSize: 17, fontWeight: 600, color: "var(--text-primary)", marginBottom: 6 }}>
          {t(titleKey ?? "platform.share.title", "分享平台")} · {title}
        </div>

        {/* 安全警示 */}
        <div
          style={{
            fontSize: 12.5,
            color: "var(--color-danger)",
            lineHeight: 1.55,
            marginBottom: 14,
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
          {FORMATS.map((f) => (
            <button
              key={f}
              type="button"
              className="btn"
              style={fmtTabStyle(format === f)}
              onClick={() => setFormat(f)}
            >
              {t(`platform.share.format.${f}`, f.toUpperCase())}
            </button>
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

        {/* 操作 */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 18, flexWrap: "wrap" }}>
          <button className="btn btn-ghost" style={{ fontSize: 13, padding: "6px 14px" }} onClick={onClose}>
            {t("action.close", "关闭")}
          </button>
          {urlScheme && (
            <button
              className="btn btn-ghost"
              style={{ fontSize: 13, padding: "6px 14px", marginRight: "auto" }}
              onClick={() => void copyUrl()}
            >
              {t(copyUrlKey ?? "mcp.share.copyUrl", "复制为 aidog:// 链接")}
            </button>
          )}
          <button
            className="btn btn-primary"
            style={{ fontSize: 13, padding: "6px 14px", minWidth: 96 }}
            onClick={() => void copy(false)}
          >
            {copied ? t("platform.share.copiedBtn", "已复制") : t("platform.share.copyBtn", "复制")}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
