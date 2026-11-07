// ─── MCP 工具 provider 设置（magpie 票 11）────────────
// 设置键 scope=mcp_tools / key=settings（票 10 落的形状：search + image/video）。
// 本节只编辑 search（provider / 多 key 轮换 / enabled / SearXNG 多实例 URL），
// image/video 原样保留透传（票 12）。保存走通用 settingsApi（settings_set），
// keys 数组已被后端 SECRET_PATHS 脱敏（***）与回填覆盖（secret_fields.rs）。
import { useCallback, useEffect, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { settingsApi } from "../../services/api/settings";
import { Button } from "@/components/ui/button";

// spec D5a 的 9 家搜索 provider（与 aidog_core SEARCH_PROVIDERS 词表一致）。
const PROVIDERS = [
  "tavily", "brave", "exa", "firecrawl", "searxng",
  "youcom", "deepseek", "doubao", "tencent_yuanbao",
] as const;
// 未能从一手文档确认公开搜索 API 的 4 家：不猜端点，调用回显式 unverified 错误。
const UNVERIFIED = new Set(["youcom", "deepseek", "doubao", "tencent_yuanbao"]);

const providerLabel = (p: string, t: TFunction): string => {
  switch (p) {
    case "doubao": return t("mcp.tools.providerDoubao", "豆包（Doubao）");
    case "tencent_yuanbao": return t("mcp.tools.providerYuanbao", "腾讯元宝（Yuanbao）");
    default: return p;
  }
};

const linesOf = (s: string): string[] =>
  s.split("\n").map((l) => l.trim()).filter((l) => l !== "");

export function McpToolsSection() {
  const { t } = useTranslation();
  const [loadError, setLoadError] = useState("");
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [provider, setProvider] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [keysText, setKeysText] = useState("");
  const [urlsText, setUrlsText] = useState("");
  const [image, setImage] = useState<Record<string, unknown> | null>(null);
  const [video, setVideo] = useState<Record<string, unknown> | null>(null);

  const load = useCallback(async () => {
    try {
      const v = await settingsApi.get("mcp_tools", "settings");
      const search = (v?.search ?? {}) as Record<string, any>;
      setProvider(typeof search.provider === "string" ? search.provider : "");
      setEnabled(search.enabled !== false);
      setKeysText(Array.isArray(search.keys) ? search.keys.join("\n") : "");
      setUrlsText(Array.isArray(search.searxng_base_urls) ? search.searxng_base_urls.join("\n") : "");
      setImage((v?.image as Record<string, unknown> | null) ?? null);
      setVideo((v?.video as Record<string, unknown> | null) ?? null);
      setLoadError("");
    } catch (e) {
      setLoadError(String(e));
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const save = async () => {
    setSaved(false);
    setSaveError("");
    try {
      await settingsApi.set("mcp_tools", "settings", {
        search: {
          provider,
          keys: linesOf(keysText),
          enabled,
          searxng_base_urls: provider === "searxng" ? linesOf(urlsText) : [],
        },
        ...(image ? { image } : {}),
        ...(video ? { video } : {}),
      });
      setSaved(true);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  return (
    <div
      data-testid="mcp-tools-section"
      className="glass-surface"
      style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}
    >
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("mcp.tools.title", "MCP 工具 provider")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("mcp.tools.desc", "配置 web_search 联网搜索的 provider、API key 与启用开关。")}
        </div>
      </div>

      {loadError && <div style={{ color: "var(--danger)", fontSize: 12 }} data-testid="mcp-tools-load-error">{loadError}</div>}

      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        {t("mcp.tools.provider", "搜索 provider")}
        <select
          data-testid="mcp-tools-provider"
          value={provider}
          onChange={(e) => { setProvider(e.target.value); setSaved(false); }}
        >
          <option value="">{t("mcp.tools.providerNone", "未配置")}</option>
          {PROVIDERS.map((p) => (
            <option key={p} value={p}>
              {providerLabel(p, t)}
              {UNVERIFIED.has(p) ? ` ${t("mcp.tools.unverifiedBadge", "（未验证）")}` : ""}
            </option>
          ))}
        </select>
      </label>

      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        {t("mcp.tools.enabled", "启用搜索")}
        <input
          type="checkbox"
          data-testid="mcp-tools-enabled"
          checked={enabled}
          onChange={(e) => { setEnabled(e.target.checked); setSaved(false); }}
        />
      </label>

      <label style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12 }}>
        <span>{t("mcp.tools.keys", "API key（每行一个，多 key 自动轮换）")}</span>
        <textarea
          data-testid="mcp-tools-keys"
          rows={3}
          value={keysText}
          onChange={(e) => { setKeysText(e.target.value); setSaved(false); }}
          style={{ width: "100%", boxSizing: "border-box" }}
        />
      </label>

      {provider === "searxng" && (
        <label style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12 }}>
          <span>{t("mcp.tools.searxngUrls", "SearXNG 实例 URL（每行一个，按序尝试）")}</span>
          <textarea
            data-testid="mcp-tools-searxng-urls"
            rows={2}
            value={urlsText}
            onChange={(e) => { setUrlsText(e.target.value); setSaved(false); }}
            style={{ width: "100%", boxSizing: "border-box" }}
          />
        </label>
      )}

      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <Button data-testid="mcp-tools-save" onClick={() => void save()}>
          {t("mcp.tools.save", "保存")}
        </Button>
        {saved && <span style={{ color: "var(--success)", fontSize: 12 }} data-testid="mcp-tools-saved">{t("mcp.tools.saved", "已保存")}</span>}
        {saveError && <span style={{ color: "var(--danger)", fontSize: 12 }} data-testid="mcp-tools-save-error">{saveError}</span>}
      </div>
    </div>
  );
}
