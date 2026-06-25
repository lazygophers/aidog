// ─── 平台「智能识别」弹窗 ──────────────────────────
// 读剪贴板 / 手动粘贴杂乱文案 → 解析 base_url / 平台 / apikey → 用户确认后填入添加表单。
// 解析逻辑见 utils/platformPaste.ts（纯函数）。视觉沿用 glass-elevated overlay 范式。

import { type CSSProperties, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import {
  parsePlatformPaste,
  type PastePresetRef,
  type ParsedProtocol,
} from "../../utils/platformPaste";
import { platformApi, type SharePlatform } from "../../services/api";

export interface SmartPasteApplyResult {
  platform: { value: string; label: string; codingPlan?: boolean } | null;
  /** 选中的 base_url（按协议类型多选，每类型最多一个）。每项 → 一个 endpoint。 */
  baseUrls: { url: string; protocol: ParsedProtocol }[];
  apiKey: string;
  /** 命中 aidog 平台分享串（YAML / JSON / Base64）时携带完整配置对象，调用方整体灌表单。
   *  存在时优先于零散 platform/baseUrls/apiKey（后者作为非分享文本的杂乱解析回退）。 */
  fullShare?: SharePlatform;
  /** 从文案中识别到的过期时间（毫秒时间戳，0/null = 未识别）。
   *  社区分享帖常见「即将过期 06-28 23:59」格式。 */
  expiresAt?: number;
}

export interface SmartPasteModalProps {
  /** Platforms.tsx 的 PLATFORM_PRESETS（须为稳定引用，避免触发解析循环）。 */
  presets: PastePresetRef[];
  onApply: (r: SmartPasteApplyResult) => void;
  onClose: () => void;
  /** 可选：跳过识别直接进空表单（主列表「添加平台」直达弹窗时的手动入口）。 */
  onManualEntry?: () => void;
}

const PROTO_LABEL: Record<ParsedProtocol, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  gemini: "Gemini",
  unknown: "URL",
};

export function SmartPasteModal({ presets, onApply, onClose, onManualEntry }: SmartPasteModalProps) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [selKey, setSelKey] = useState("");
  // 多选 base_url：按协议类型分组，每类型最多选一个（不同类型可并存 → 多 endpoint）。
  const [selUrls, setSelUrls] = useState<string[]>([]);

  const parsed = useMemo(() => parsePlatformPaste(text, presets), [text, presets]);
  // 命中 aidog 平台分享串（YAML / JSON / Base64）→ 整体灌表单（优先于杂乱解析）。
  const [share, setShare] = useState<SharePlatform | null>(null);

  // 文本变化时尝试解析 aidog 分享串：先原文（serde_yml 兼容 YAML/JSON），失败再试 base64 解码后解析。
  // 非分享文本两路都失败 → share=null，回退原杂乱解析（无回归）。
  useEffect(() => {
    let cancelled = false;
    const trimmed = text.trim();
    if (!trimmed) {
      setShare(null);
      return;
    }
    const tryParse = async () => {
      const candidates = [trimmed];
      // base64 包裹的 YAML：仅当文本像单段 base64 时才尝试解码（避免误伤普通文本）。
      if (/^[A-Za-z0-9+/=\s]+$/.test(trimmed) && !trimmed.includes(":")) {
        try {
          const bin = atob(trimmed.replace(/\s+/g, ""));
          const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
          candidates.push(new TextDecoder().decode(bytes));
        } catch {
          /* 非合法 base64 → 跳过 */
        }
      }
      for (const c of candidates) {
        try {
          const r = await platformApi.shareParse(c);
          if (!cancelled) setShare(r);
          return;
        } catch {
          /* 非分享串 → 试下一候选 */
        }
      }
      if (!cancelled) setShare(null);
    };
    void tryParse();
    return () => {
      cancelled = true;
    };
  }, [text]);

  // 渲染完成自动读剪贴板填入（仅首次 mount 且 text 空；失败静默兜底手动粘贴）。
  // 走 Tauri 插件非 navigator.clipboard：macOS WKWebView 无手势激活时 navigator API 被拒静默失败，
  // 而 Tauri Rust 侧 read_text 走权限系统无需手势，可靠。
  useEffect(() => {
    let cancelled = false;
    readText()
      .then((clip) => {
        if (!cancelled && clip && clip.trim() && text === "") setText(clip);
      })
      .catch(() => {
        /* 权限不足 / 无剪贴板 → 静默，手动粘贴兜底 */
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 默认选中：每个协议类型的首个 url（不同类型并存，同类型只取第一个）。
  useEffect(() => {
    const byProto = new Map<ParsedProtocol, string>();
    for (const b of parsed.baseUrls) {
      if (!byProto.has(b.protocol)) byProto.set(b.protocol, b.url);
    }
    setSelUrls(Array.from(byProto.values()));
    setSelKey(parsed.apiKeys[0] ?? "");
  }, [parsed]);

  // 切换某 url 选中态：同协议类型内互斥（选新自动替旧），不同类型可并存。
  const toggleUrl = (url: string, protocol: ParsedProtocol) => {
    setSelUrls((prev) => {
      if (prev.includes(url)) {
        return prev.filter((u) => u !== url);
      }
      // 剔除同协议已选，加入新选
      const sameProtoUrl = parsed.baseUrls.find((b) => b.protocol === protocol && prev.includes(b.url))?.url;
      return sameProtoUrl ? prev.map((u) => (u === sameProtoUrl ? url : u)) : [...prev, url];
    });
  };

  const hasResult = parsed.apiKeys.length > 0 || parsed.baseUrls.length > 0 || !!parsed.platform;
  const hasExpiry = !!parsed.expiresAt && parsed.expiresAt > 0;
  const canApply = !!share || !!(selKey || selUrls.length > 0 || parsed.platform);

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

        {/* aidog 平台分享串命中 → 整体灌表单（优先于杂乱解析） */}
        {share && (
          <div
            style={{
              marginTop: 16,
              padding: "12px 14px",
              borderRadius: "var(--radius-sm)",
              background: "var(--accent-subtle)",
              border: "1px solid var(--accent)",
              display: "flex",
              flexDirection: "column",
              gap: 4,
            }}
          >
            <div style={{ fontSize: 13, fontWeight: 600, color: "var(--accent)" }}>
              {t("platform.paste.shareDetected", "已识别 AiDog 平台分享")}
            </div>
            <div style={{ fontSize: 12.5, color: "var(--text-secondary)" }}>
              {share.name} · {share.platform_type.toUpperCase()}
            </div>
            <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
              {t("platform.paste.shareDetectedHint", "点击下方按钮将完整配置（含 API Key）灌入表单。")}
            </div>
          </div>
        )}

        {/* 识别结果（非分享串的杂乱文本解析） */}
        {!share && (
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

          {/* Base URL（按协议类型多选：每类型最多一个，不同类型并存 → 多 endpoint） */}
          {parsed.baseUrls.length > 0 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div style={{ ...labelStyle, display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span>{t("platform.paste.baseUrl", "Base URL")}</span>
                <span style={{ fontSize: 10.5, fontWeight: 500, color: "var(--text-tertiary)", textTransform: "none", letterSpacing: 0 }}>
                  {t("platform.paste.baseUrlMultiHint", "不同类型可各选一个")}
                </span>
              </div>
              {parsed.baseUrls.map((b) => {
                const checked = selUrls.includes(b.url);
                return (
                  <label
                    key={b.url}
                    style={{ ...optRow, borderColor: checked ? "var(--accent)" : "var(--border)" }}
                  >
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => toggleUrl(b.url, b.protocol)}
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
                );
              })}
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

          {/* 过期时间（社区分享帖常见「即将过期 06-28 23:59」） */}
          {hasExpiry && (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div style={labelStyle}>{t("platform.expiresAt", "过期时间")}</div>
              <div style={{ ...optRow, cursor: "default", borderColor: "var(--accent)" }}>
                <span style={{ color: "var(--accent)", fontWeight: 600 }}>
                  {new Date(parsed.expiresAt!).toLocaleString()}
                </span>
              </div>
            </div>
          )}
        </div>
        )}

        {/* 操作 */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 20 }}>
          {onManualEntry && (
            <button className="btn btn-ghost" style={{ fontSize: 13, padding: "6px 14px" }} onClick={onManualEntry}>
              {t("platform.paste.manualEntry", "手动填写")}
            </button>
          )}
          <button className="btn btn-ghost" style={{ fontSize: 13, padding: "6px 14px" }} onClick={onClose}>
            {t("action.cancel", "取消")}
          </button>
          <button
            className="btn btn-primary"
            style={{ fontSize: 13, padding: "6px 14px", minWidth: 96 }}
            disabled={!canApply}
            onClick={() => {
              if (share) {
                onApply({ platform: null, baseUrls: [], apiKey: "", fullShare: share });
                onClose();
                return;
              }
              const selected = parsed.baseUrls
                .filter((b) => selUrls.includes(b.url))
                .map((b) => ({ url: b.url, protocol: b.protocol }));
              onApply({
                platform: parsed.platform,
                baseUrls: selected,
                apiKey: selKey,
                expiresAt: parsed.expiresAt ?? 0,
              });
              onClose();
            }}
          >
            {t("platform.paste.apply", "填入表单")}
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
}
