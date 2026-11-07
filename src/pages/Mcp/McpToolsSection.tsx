// ─── MCP 工具 provider 设置（magpie 票 11 搜索 / 票 12 生图生视频 + 一键安装）──
// 设置键 scope=mcp_tools / key=settings。本节编辑 search（provider / 多 key 轮换 /
// enabled / SearXNG 多实例 URL）与 image / video provider（base_url / api_key / model，
// OpenAI images 兼容端点）；「一键写入」把 `aidog-kernel mcp`（stdio）写进本机
// Claude Code / Codex 的 mcpServers（复用 aidog_mcp 机制，既有条目保留）。
// 保存走通用 settingsApi（settings_set），api_key/keys 数组已被后端 SECRET_PATHS
// 脱敏（***）与回填覆盖（secret_fields.rs，settings_set 已登记 RESTORERS）。
import { useCallback, useEffect, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { settingsApi } from "../../services/api/settings";
import { mcpApi } from "../../services/api/mcp";
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

/** 生图 / 生视频 provider 配置（MediaProviderConfig，snake_case 与后端对齐）。 */
type MediaCfg = { base_url: string; api_key: string; model: string };
const EMPTY_MEDIA: MediaCfg = { base_url: "", api_key: "", model: "" };
const readMedia = (v: unknown): MediaCfg => {
  const m = (v ?? {}) as Record<string, unknown>;
  return {
    base_url: typeof m.base_url === "string" ? m.base_url : "",
    api_key: typeof m.api_key === "string" ? m.api_key : "",
    model: typeof m.model === "string" ? m.model : "",
  };
};
const hasMedia = (c: MediaCfg): boolean =>
  c.base_url !== "" || c.api_key !== "" || c.model !== "";

export function McpToolsSection() {
  const { t } = useTranslation();
  const [loadError, setLoadError] = useState("");
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [provider, setProvider] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [keysText, setKeysText] = useState("");
  const [urlsText, setUrlsText] = useState("");
  const [image, setImage] = useState<MediaCfg>(EMPTY_MEDIA);
  const [video, setVideo] = useState<MediaCfg>(EMPTY_MEDIA);
  const [installing, setInstalling] = useState(false);
  const [installMsg, setInstallMsg] = useState("");
  const [installOk, setInstallOk] = useState(false);

  const load = useCallback(async () => {
    try {
      const v = await settingsApi.get("mcp_tools", "settings");
      const search = (v?.search ?? {}) as Record<string, any>;
      setProvider(typeof search.provider === "string" ? search.provider : "");
      setEnabled(search.enabled !== false);
      setKeysText(Array.isArray(search.keys) ? search.keys.join("\n") : "");
      setUrlsText(Array.isArray(search.searxng_base_urls) ? search.searxng_base_urls.join("\n") : "");
      setImage(readMedia(v?.image));
      setVideo(readMedia(v?.video));
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
        ...(hasMedia(image) ? { image } : {}),
        ...(hasMedia(video) ? { video } : {}),
      });
      setSaved(true);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  const install = async () => {
    setInstalling(true);
    setInstallMsg("");
    try {
      const r = await mcpApi.selfInstall();
      setInstallOk(true);
      setInstallMsg(
        t("mcp.tools.installed", "已写入 {{name}} 的 {{agents}} 配置，重启对应 agent 生效", {
          name: r.name,
          agents: r.agents.join(", "),
        }),
      );
    } catch (e) {
      setInstallOk(false);
      setInstallMsg(String(e));
    } finally {
      setInstalling(false);
    }
  };

  const mediaSection = (
    title: string,
    kind: "image" | "video",
    cfg: MediaCfg,
    onChange: (c: MediaCfg) => void,
  ) => (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ fontSize: 12, fontWeight: 600 }}>{title}</div>
      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        base_url
        <input
          data-testid={`mcp-tools-${kind}-base-url`}
          value={cfg.base_url}
          onChange={(e) => { onChange({ ...cfg, base_url: e.target.value }); setSaved(false); }}
          placeholder="https://api.openai.com/v1"
          style={{ flex: 1 }}
        />
      </label>
      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        {t("mcp.tools.apiKey", "API key")}
        <input
          data-testid={`mcp-tools-${kind}-api-key`}
          value={cfg.api_key}
          onChange={(e) => { onChange({ ...cfg, api_key: e.target.value }); setSaved(false); }}
          style={{ flex: 1 }}
        />
      </label>
      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        {t("mcp.tools.model", "模型名")}
        <input
          data-testid={`mcp-tools-${kind}-model`}
          value={cfg.model}
          onChange={(e) => { onChange({ ...cfg, model: e.target.value }); setSaved(false); }}
          style={{ flex: 1 }}
        />
      </label>
    </div>
  );

  return (
    <div
      data-testid="mcp-tools-section"
      className="glass-surface"
      style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}
    >
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("mcp.tools.title", "MCP 工具 provider")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("mcp.tools.desc", "配置 web_search 联网搜索、generate_image 生图与 generate_video 的 provider。")}
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

      <div style={{ fontSize: 12, fontWeight: 600 }}>
        {t("mcp.tools.mediaTitle", "生图 / 生视频 provider（OpenAI images 兼容端点）")}
      </div>
      {mediaSection(t("mcp.tools.imageTitle", "生图（generate_image）"), "image", image, setImage)}
      {mediaSection(t("mcp.tools.videoTitle", "生视频（generate_video，首版未实现）"), "video", video, setVideo)}

      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <Button data-testid="mcp-tools-save" onClick={() => void save()}>
          {t("mcp.tools.save", "保存")}
        </Button>
        {saved && <span style={{ color: "var(--success)", fontSize: 12 }} data-testid="mcp-tools-saved">{t("mcp.tools.saved", "已保存")}</span>}
        {saveError && <span style={{ color: "var(--danger)", fontSize: 12 }} data-testid="mcp-tools-save-error">{saveError}</span>}
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <Button data-testid="mcp-tools-install" disabled={installing} onClick={() => void install()}>
          {t("mcp.tools.install", "一键写入本机 agent 配置")}
        </Button>
        <span className="text-secondary" style={{ fontSize: 12 }}>
          {t(
            "mcp.tools.installHint",
            "把 aidog-kernel mcp 以 stdio 写进本机 Claude Code（~/.claude.json）与 Codex（~/.codex/config.toml）的 mcpServers；既有条目保留，只更新同名 aidog 条目。",
          )}
        </span>
      </div>
      {installMsg && (
        <div
          data-testid="mcp-tools-install-result"
          style={{ color: installOk ? "var(--success)" : "var(--danger)", fontSize: 12 }}
        >
          {installMsg}
        </div>
      )}
    </div>
  );
}
