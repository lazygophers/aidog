// ─── 平台「智能识别」弹窗 ──────────────────────────
// 读剪贴板 / 手动粘贴杂乱文案 → 解析 base_url / 平台 / apikey → 用户确认后填入添加表单。
// 解析逻辑见 utils/platformPaste.ts（纯函数）。视觉沿用 glass-elevated overlay 范式。

import { type CSSProperties, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  parsePlatformPaste,
  type PastePresetRef,
  type ParsedProtocol,
} from "../../utils/platformPaste";

export interface SmartPasteApplyResult {
  platform: { value: string; label: string } | null;
  baseUrl: string;
  /** 选中 base_url 的协议倾向（供调用方写入正确 endpoint）。 */
  baseUrlProtocol: ParsedProtocol;
  apiKey: string;
}

export interface SmartPasteModalProps {
  /** Platforms.tsx 的 PLATFORM_PRESETS（须为稳定引用，避免触发解析循环）。 */
  presets: PastePresetRef[];
  onApply: (r: SmartPasteApplyResult) => void;
  onClose: () => void;
}

const PROTO_LABEL: Record<ParsedProtocol, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  gemini: "Gemini",
  unknown: "URL",
};

export function SmartPasteModal({ presets, onApply, onClose }: SmartPasteModalProps) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [selKey, setSelKey] = useState("");
  const [selUrl, setSelUrl] = useState("");
  const [clipErr, setClipErr] = useState(false);

  const parsed = useMemo(() => parsePlatformPaste(text, presets), [text, presets]);

  // 默认选中首个候选（解析结果变化时重置）
  useEffect(() => {
    setSelKey(parsed.apiKeys[0] ?? "");
    setSelUrl(parsed.baseUrls[0]?.url ?? "");
  }, [parsed]);

  // 打开时 best-effort 读剪贴板（拒绝 / 不支持 → 手动粘贴）
  useEffect(() => {
    (async () => {
      try {
        const clip = await navigator.clipboard.readText();
        if (clip && clip.trim()) setText(clip);
      } catch {
        setClipErr(true);
      }
    })();
  }, []);

  const hasResult = parsed.apiKeys.length > 0 || parsed.baseUrls.length > 0 || !!parsed.platform;
  const canApply = !!(selKey || selUrl || parsed.platform);

  const readClipboard = async () => {
    try {
      const clip = await navigator.clipboard.readText();
      if (clip && clip.trim()) {
        setText(clip);
        setClipErr(false);
      }
    } catch {
      setClipErr(true);
    }
  };

  const labelStyle: CSSProperties = {
    fontSize: 12,
    fontWeight: 600,
    color: "var(--text-secondary)",
    textTransform: "uppercase",
    letterSpacing: 0.4,
  };
  const optRow: CSSProperties = {
    display: "flex",
    alignItems: "center",
    gap: 8,
    padding: "7px 10px",
    borderRadius: "var(--radius-sm)",
    background: "var(--bg-glass)",
    border: "1px solid var(--border)",
    cursor: "pointer",
    fontSize: 13,
    wordBreak: "break-all",
  };

  return (
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
          width: 540,
          maxWidth: "92vw",
          maxHeight: "86vh",
          display: "flex",
          flexDirection: "column",
          borderRadius: "var(--radius-lg)",
          animation: "fadeIn 200ms ease both",
          padding: "22px 24px",
          overflowY: "auto",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ fontSize: 17, fontWeight: 600, color: "var(--text-primary)", marginBottom: 6 }}>
          {t("platform.paste.title", "智能识别")}
        </div>
        <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.55, marginBottom: 14 }}>
          {t("platform.paste.hint", "粘贴分享文案，自动识别 Base URL、平台与 API Key（base64 自动解码）。")}
        </div>

        <textarea
          className="input"
          style={{
            width: "100%",
            minHeight: 130,
            resize: "vertical",
            fontFamily: "var(--font-mono, monospace)",
            fontSize: 12.5,
            lineHeight: 1.5,
          }}
          placeholder={t("platform.paste.placeholder", "在此粘贴文案…")}
          value={text}
          onChange={(e) => setText(e.target.value)}
          autoFocus
        />

        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 8 }}>
          <button className="btn btn-ghost" style={{ fontSize: 12, padding: "4px 10px" }} onClick={readClipboard}>
            {t("platform.paste.readClipboard", "读取剪贴板")}
          </button>
          {clipErr && (
            <span style={{ fontSize: 11.5, color: "var(--text-secondary)" }}>
              {t("platform.paste.clipDenied", "无法读取剪贴板，请手动粘贴")}
            </span>
          )}
        </div>

        {/* 识别结果 */}
        <div style={{ marginTop: 16, display: "flex", flexDirection: "column", gap: 14 }}>
          <div style={labelStyle}>{t("platform.paste.detected", "识别结果")}</div>

          {!hasResult && (
            <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>
              {t("platform.paste.empty", "粘贴文案后自动识别。")}
            </div>
          )}

          {/* 平台 */}
          {hasResult && (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div style={labelStyle}>{t("platform.paste.platform", "平台")}</div>
              {parsed.platform ? (
                <div style={{ ...optRow, cursor: "default", borderColor: "var(--accent)" }}>
                  <span style={{ color: "var(--accent)", fontWeight: 600 }}>{parsed.platform.label}</span>
                </div>
              ) : (
                <div style={{ fontSize: 12.5, color: "var(--text-secondary)" }}>
                  {t("platform.paste.noPlatform", "未匹配内置平台，将保持当前平台选择不变。")}
                </div>
              )}
            </div>
          )}

          {/* Base URL */}
          {parsed.baseUrls.length > 0 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div style={labelStyle}>{t("platform.paste.baseUrl", "Base URL")}</div>
              {parsed.baseUrls.map((b) => (
                <label
                  key={b.url}
                  style={{ ...optRow, borderColor: selUrl === b.url ? "var(--accent)" : "var(--border)" }}
                >
                  <input
                    type="radio"
                    name="paste-url"
                    checked={selUrl === b.url}
                    onChange={() => setSelUrl(b.url)}
                  />
                  <span
                    style={{
                      flexShrink: 0,
                      fontSize: 10.5,
                      padding: "1px 6px",
                      borderRadius: 4,
                      background: "var(--accent-subtle)",
                      color: "var(--accent)",
                    }}
                  >
                    {PROTO_LABEL[b.protocol]}
                  </span>
                  <span style={{ color: "var(--text-primary)" }}>{b.url}</span>
                </label>
              ))}
            </div>
          )}

          {/* API Key */}
          {parsed.apiKeys.length > 0 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div style={labelStyle}>{t("platform.paste.apiKey", "API Key")}</div>
              {parsed.apiKeys.map((k) => (
                <label
                  key={k}
                  style={{ ...optRow, borderColor: selKey === k ? "var(--accent)" : "var(--border)" }}
                >
                  <input
                    type="radio"
                    name="paste-key"
                    checked={selKey === k}
                    onChange={() => setSelKey(k)}
                  />
                  <span style={{ color: "var(--text-primary)", fontFamily: "var(--font-mono, monospace)" }}>{k}</span>
                </label>
              ))}
            </div>
          )}
        </div>

        {/* 操作 */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 20 }}>
          <button className="btn btn-ghost" style={{ fontSize: 13, padding: "6px 14px" }} onClick={onClose}>
            {t("action.cancel", "取消")}
          </button>
          <button
            className="btn btn-primary"
            style={{ fontSize: 13, padding: "6px 14px", minWidth: 96 }}
            disabled={!canApply}
            onClick={() => {
              const urlObj = parsed.baseUrls.find((b) => b.url === selUrl);
              onApply({
                platform: parsed.platform,
                baseUrl: selUrl,
                baseUrlProtocol: urlObj?.protocol ?? "unknown",
                apiKey: selKey,
              });
              onClose();
            }}
          >
            {t("platform.paste.apply", "填入表单")}
          </button>
        </div>
      </div>
    </div>
  );
}
