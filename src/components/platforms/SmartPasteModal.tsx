// ─── 平台「智能识别」弹窗 ──────────────────────────
// 读剪贴板 / 手动粘贴杂乱文案 → 解析 base_url / 平台 / apikey → 用户确认后填入添加表单。
// 解析逻辑见 utils/platformPaste.ts（纯函数）。视觉沿用 glass-elevated overlay 范式。
// Dialog 走 Radix Portal（替代 shared/Modal，liquid glass 居中由 Portal 保证）。

import { type CSSProperties, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import {
  parsePlatformPaste,
  type PastePresetRef,
  type ParsedProtocol,
} from "../../utils/platformPaste";
import { platformApi, type SharePlatform } from "../../services/api";
import { getProtocolLabel } from "../../domains/platforms/defaults";
import { formatDateTime } from "../../utils/formatters";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Checkbox } from "@/components/ui/checkbox";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export interface SmartPasteApplyResult {
  platform: { value: string; label: string; codingPlan?: boolean } | null;
  /** 选中的 base_url（按协议类型多选，每类型最多一个）。每项 → 一个 endpoint。 */
  baseUrls: { url: string; protocol: ParsedProtocol }[];
  /** 选中的 apikey 列表（长度 1 = 单平台旧路径，>1 = 批量创建 N 平台）。
   *  SmartPaste 路径从单选 radio 升级为多选 checkbox（多 key 时），向后兼容单 key 场景。 */
  apiKeys: string[];
  /** 命中 aidog 平台分享串（YAML / JSON / Base64）时携带完整配置对象，调用方整体灌表单。
   *  存在时优先于零散 platform/baseUrls/apiKeys（后者作为非分享文本的杂乱解析回退）。 */
  fullShare?: SharePlatform;
  /** 从文案中识别到的过期时间（毫秒时间戳，0/null = 未识别）。
   *  社区分享帖常见「即将过期 06-28 23:59」格式。 */
  expiresAt?: number;
}

export interface SmartPasteModalProps {
  /** Platforms.tsx 的 PLATFORM_PRESETS（须为稳定引用，避免触发解析循环）。 */
  presets: PastePresetRef[];
  onApply: (r: SmartPasteApplyResult) => void | Promise<void>;
  onClose: () => void;
  /** 可选：跳过识别直接进空表单（主列表「添加平台」直达弹窗时的手动入口）。 */
  onManualEntry?: () => void;
  /** 可选：预填文本（如 aidog:// deep-link 导入场景）。提供时以此初始化并跳过自动读剪贴板。 */
  initialText?: string;
}

const PROTO_LABEL: Record<ParsedProtocol, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  gemini: "Gemini",
  unknown: "URL",
};

export function SmartPasteModal({ presets, onApply, onClose, onManualEntry, initialText }: SmartPasteModalProps) {
  const { t, i18n } = useTranslation();
  const [text, setText] = useState(initialText ?? "");
  const [selKeys, setSelKeys] = useState<string[]>([]);
  const [selUrls, setSelUrls] = useState<string[]>([]);

  const parsed = useMemo(() => parsePlatformPaste(text, presets), [text, presets]);
  const [share, setShare] = useState<SharePlatform | null>(null);
  const [protocolLabel, setProtocolLabel] = useState("");

  useEffect(() => {
    let cancelled = false;
    const trimmed = text.trim();
    if (!trimmed) {
      setShare(null);
      return;
    }
    const tryParse = async () => {
      const candidates = [trimmed];
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

  useEffect(() => {
    if (initialText) return;
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

  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (!share?.platform_type) return;
      const label = await getProtocolLabel(share.platform_type, i18n.language);
      if (!cancelled) setProtocolLabel(label);
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [i18n.language, share?.platform_type]);

  useEffect(() => {
    const byProto = new Map<ParsedProtocol, string>();
    for (const b of parsed.baseUrls) {
      if (!byProto.has(b.protocol)) byProto.set(b.protocol, b.url);
    }
    setSelUrls(Array.from(byProto.values()));
    setSelKeys(parsed.apiKeys.slice());
  }, [parsed]);

  /** 切换某 key 的选中态（多选场景）。 */
  const toggleKey = (k: string) => {
    setSelKeys(prev => prev.includes(k) ? prev.filter(x => x !== k) : [...prev, k]);
  };

  const toggleUrl = (url: string, protocol: ParsedProtocol) => {
    setSelUrls((prev) => {
      if (prev.includes(url)) {
        return prev.filter((u) => u !== url);
      }
      const sameProtoUrl = parsed.baseUrls.find((b) => b.protocol === protocol && prev.includes(b.url))?.url;
      return sameProtoUrl ? prev.map((u) => (u === sameProtoUrl ? url : u)) : [...prev, url];
    });
  };

  const hasResult = parsed.apiKeys.length > 0 || parsed.baseUrls.length > 0 || !!parsed.platform;
  const hasExpiry = !!parsed.expiresAt && parsed.expiresAt > 0;
  const hasSelKey = parsed.apiKeys.length <= 1 ? !!selKeys[0] : selKeys.length > 0;
  const canApply = !!share || !!(hasSelKey || selUrls.length > 0 || parsed.platform);

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
    <Dialog open onOpenChange={(next) => { if (!next) onClose(); }}>
      <DialogContent
        className="glass-elevated"
        style={{ maxWidth: 540, padding: "22px 24px", maxHeight: "86vh", overflowY: "auto" }}
      >
        <DialogHeader>
          <DialogTitle style={{ fontSize: 17 }}>
            {t("platform.paste.title", "智能识别")}
          </DialogTitle>
          <DialogDescription style={{ lineHeight: 1.55 }}>
            {t("platform.paste.hint", "粘贴分享文案，自动识别 Base URL、平台与 API Key（base64 自动解码）。")}
          </DialogDescription>
        </DialogHeader>

        <Textarea
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
              {share.name} · {protocolLabel || share.platform_type}
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
                    <Checkbox
                      checked={checked}
                      onCheckedChange={() => toggleUrl(b.url, b.protocol)}
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

          {/* API Key（单 key = radio 单选，向后兼容；多 key = checkbox 多选，默认全选 → 批量创建） */}
          {parsed.apiKeys.length > 0 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div style={{ ...labelStyle, display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span>{t("platform.paste.apiKey", "API Key")}</span>
                {parsed.apiKeys.length > 1 && (
                  <span style={{ fontSize: 10.5, fontWeight: 500, color: "var(--text-tertiary)", textTransform: "none", letterSpacing: 0 }}>
                    {t("platform.paste.apiKeyMultiHint", "已选 {{n}}/{{total}}，将批量创建", { n: selKeys.length, total: parsed.apiKeys.length })}
                  </span>
                )}
              </div>
              {/* 多 key 走 Checkbox 多选；单 key 走 RadioGroup 单选（向后兼容） */}
              {parsed.apiKeys.length > 1 ? (
                parsed.apiKeys.map((k) => {
                  const checked = selKeys.includes(k);
                  return (
                    <label
                      key={k}
                      style={{ ...optRow, borderColor: checked ? "var(--accent)" : "var(--border)" }}
                    >
                      <Checkbox
                        checked={checked}
                        onCheckedChange={() => toggleKey(k)}
                      />
                      <span style={{ color: "var(--text-primary)", fontFamily: "var(--font-mono, monospace)" }}>{k}</span>
                    </label>
                  );
                })
              ) : (
                <RadioGroup
                  value={selKeys[0] ?? ""}
                  onValueChange={(v) => setSelKeys([v])}
                  style={{ display: "flex", flexDirection: "column", gap: 6 }}
                >
                  {parsed.apiKeys.map((k) => {
                    const checked = selKeys[0] === k;
                    return (
                      <label
                        key={k}
                        style={{ ...optRow, borderColor: checked ? "var(--accent)" : "var(--border)" }}
                      >
                        <RadioGroupItem value={k} id={`paste-key-${k}`} />
                        <span style={{ color: "var(--text-primary)", fontFamily: "var(--font-mono, monospace)" }}>{k}</span>
                      </label>
                    );
                  })}
                </RadioGroup>
              )}
            </div>
          )}

          {/* 过期时间（社区分享帖常见「即将过期 06-28 23:59」） */}
          {hasExpiry && (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div style={labelStyle}>{t("platform.expiresAt", "过期时间")}</div>
              <div style={{ ...optRow, cursor: "default", borderColor: "var(--accent)" }}>
                <span style={{ color: "var(--accent)", fontWeight: 600 }}>
                  {formatDateTime(parsed.expiresAt!) || "-"}
                </span>
              </div>
            </div>
          )}
        </div>
        )}

        <DialogFooter>
          {onManualEntry && (
            <Button variant="ghost" style={{ fontSize: 13, padding: "6px 14px" }} onClick={onManualEntry}>
              {t("platform.paste.manualEntry", "手动填写")}
            </Button>
          )}
          <Button variant="ghost" style={{ fontSize: 13, padding: "6px 14px" }} onClick={onClose}>
            {t("action.cancel", "取消")}
          </Button>
          <Button
            style={{ fontSize: 13, padding: "6px 14px", minWidth: 96 }}
            disabled={!canApply}
            onClick={() => {
              if (share) {
                onApply({ platform: null, baseUrls: [], apiKeys: [], fullShare: share });
                onClose();
                return;
              }
              const selected = parsed.baseUrls
                .filter((b) => selUrls.includes(b.url))
                .map((b) => ({ url: b.url, protocol: b.protocol }));
              onApply({
                platform: parsed.platform,
                baseUrls: selected,
                apiKeys: selKeys,
                expiresAt: parsed.expiresAt ?? 0,
              });
              onClose();
            }}
          >
            {t("platform.paste.apply", "填入表单")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
