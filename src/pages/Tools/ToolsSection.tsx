// ─── 「扩展 → 工具」页主体：搜索（key 服务 + 免 key 引擎开关）/ 生图 ──
// 设置键 scope=mcp_tools / key=settings。本节编辑 search（provider / 多 key 轮换 /
// enabled / engines 开关 / SearXNG 实例 URL）与 image provider（base_url / api_key /
// model，OpenAI images 兼容端点）；「一键写入」把 AiDog 的 HTTP MCP 地址（+ 分组 Bearer）写进本机
// Claude Code / Codex 的 mcpServers（复用 aidog_mcp 机制，既有条目保留）。
// 保存走通用 settingsApi（settings_set），api_key/keys 数组已被后端 SECRET_PATHS
// 脱敏（***）与回填覆盖（secret_fields.rs，settings_set 已登记 RESTORERS）。
import { useCallback, useEffect, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { settingsApi } from "../../services/api/settings";
import { mcpApi } from "../../services/api/mcp";
import { searchApi, type SearxInstance } from "../../services/api/search";
import { Button } from "@/components/ui/button";

// 搜索 key 服务（SearXNG 已并入下方免 key 引擎列表；词表真值源见 aidog_search KEY_SERVICES）。
const PROVIDERS = [
  "tavily", "brave", "exa", "firecrawl",
  "youcom", "deepseek", "doubao", "tencent_yuanbao",
] as const;
// 免 key 引擎清单与默认开关：与 aidog_search::engines::ENGINES 同序同值（加引擎两处同改）。
export const SEARCH_ENGINES = [
  { id: "ddg", defaultOn: true },
  { id: "ddg_lite", defaultOn: false },
  { id: "bing", defaultOn: true },
  { id: "google", defaultOn: true },
  { id: "yandex", defaultOn: false },
  { id: "sogou", defaultOn: false },
  { id: "baidu", defaultOn: false },
  { id: "so360", defaultOn: false },
  { id: "arxiv", defaultOn: true },
  { id: "crossref", defaultOn: false },
  { id: "pubmed", defaultOn: false },
  { id: "searxng", defaultOn: true },
  { id: "wikipedia", defaultOn: true },
  { id: "github", defaultOn: true },
] as const;
const defaultEngineFlags = (): Record<string, boolean> =>
  Object.fromEntries(SEARCH_ENGINES.map((e) => [e.id, e.defaultOn]));
// 未能从一手文档确认公开搜索 API 的 4 家：不猜端点，调用回显式 unverified 错误。
const UNVERIFIED = new Set(["youcom", "deepseek", "doubao", "tencent_yuanbao"]);

const providerLabel = (p: string, t: TFunction): string => {
  switch (p) {
    case "doubao": return t("tools.providerDoubao", "豆包（Doubao）");
    case "tencent_yuanbao": return t("tools.providerYuanbao", "腾讯元宝（Yuanbao）");
    default: return p;
  }
};

const linesOf = (s: string): string[] =>
  s.split("\n").map((l) => l.trim()).filter((l) => l !== "");

/** 生图 provider 配置（MediaProviderConfig，snake_case 与后端对齐）。 */
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

export function ToolsSection() {
  const { t } = useTranslation();
  const [loadError, setLoadError] = useState("");
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [provider, setProvider] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [keysText, setKeysText] = useState("");
  const [urlsText, setUrlsText] = useState("");
  const [searxList, setSearxList] = useState<SearxInstance[]>([]);
  const [searxBusy, setSearxBusy] = useState(false);
  const [searxError, setSearxError] = useState("");
  const [engines, setEngines] = useState<Record<string, boolean>>(defaultEngineFlags);
  // 过渡适配（票 02）：UI 仍是「一个 key 服务 + 引擎开关」，读写新形状；其余 key 服务与总开关原样带回，不丢 key。
  const [services, setServices] = useState<Record<string, { keys: string[] }>>({});
  const [searchEnabled, setSearchEnabled] = useState(true);
  const [image, setImage] = useState<MediaCfg>(EMPTY_MEDIA);
  const [installing, setInstalling] = useState(false);
  const [installMsg, setInstallMsg] = useState("");
  const [installOk, setInstallOk] = useState(false);

  const load = useCallback(async () => {
    try {
      const v = await settingsApi.get("mcp_tools", "settings");
      const search = (v?.search ?? {}) as Record<string, any>;
      const sources: string[] = Array.isArray(search.sources) ? search.sources : [];
      const svc = (search.services ?? {}) as Record<string, { keys?: string[] }>;
      const norm: Record<string, { keys: string[] }> = {};
      for (const [id, c] of Object.entries(svc)) norm[id] = { keys: Array.isArray(c?.keys) ? c.keys : [] };
      setServices(norm);
      setSearchEnabled(search.enabled !== false);
      const listed = PROVIDERS.find((p) => sources.includes(p));
      const p = listed ?? PROVIDERS.find((id) => (norm[id]?.keys.length ?? 0) > 0) ?? "";
      setProvider(p);
      setEnabled(listed !== undefined || p === "");
      setKeysText(p ? (norm[p]?.keys ?? []).join("\n") : "");
      // 无 sources（尚未保存过）= 全新安装，引擎取缺省开关（与后端 Default 一致）。
      setEngines(Array.isArray(search.sources)
        ? Object.fromEntries(SEARCH_ENGINES.map((e) => [e.id, sources.includes(e.id)]))
        : defaultEngineFlags());
      setUrlsText(Array.isArray(search.searxng_base_urls) ? search.searxng_base_urls.join("\n") : "");
      setImage(readMedia(v?.image));
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
          enabled: searchEnabled,
          sources: [
            ...(provider && enabled ? [provider] : []),
            ...SEARCH_ENGINES.filter((e) => engines[e.id]).map((e) => e.id),
          ],
          mode: provider && enabled ? "sequential" : "parallel",
          services: provider ? { ...services, [provider]: { keys: linesOf(keysText) } } : services,
          searxng_base_urls: linesOf(urlsText),
        },
        ...(hasMedia(image) ? { image } : {}),
      });
      setSaved(true);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  // 手动刷新 searx.space 公共列表：只在点按钮时拉一次，AiDog 不自动拉取。
  const refreshSearx = async () => {
    setSearxBusy(true);
    setSearxError("");
    try {
      setSearxList(await searchApi.searxInstances());
    } catch (e) {
      setSearxError(String(e));
    } finally {
      setSearxBusy(false);
    }
  };
  // 勾选 = 把实例 URL 追加进下方文本（随保存持久化），取消 = 移除该行。
  const toggleSearx = (url: string, on: boolean) => {
    const lines = linesOf(urlsText).filter((l) => l !== url);
    setUrlsText((on ? [...lines, url] : lines).join("\n"));
    setSaved(false);
  };

  const install = async () => {
    setInstalling(true);
    setInstallMsg("");
    try {
      const r = await mcpApi.selfInstall();
      setInstallOk(true);
      setInstallMsg(
        t("tools.installed", "已写入 {{name}} 的 {{agents}} 配置，重启对应 agent 生效", {
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
    kind: "image",
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
        {t("tools.apiKey", "API key")}
        <input
          data-testid={`mcp-tools-${kind}-api-key`}
          value={cfg.api_key}
          onChange={(e) => { onChange({ ...cfg, api_key: e.target.value }); setSaved(false); }}
          style={{ flex: 1 }}
        />
      </label>
      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        {t("tools.model", "模型名")}
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
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("tools.title", "搜索 / 生图")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("tools.desc", "配置 web_search 联网搜索与 generate_image 生图的 provider。")}
        </div>
      </div>

      {loadError && <div style={{ color: "var(--danger)", fontSize: 12 }} data-testid="mcp-tools-load-error">{loadError}</div>}

      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        {t("tools.provider", "搜索 provider")}
        <select
          data-testid="mcp-tools-provider"
          value={provider}
          onChange={(e) => { setProvider(e.target.value); setSaved(false); }}
      >
          <option value="">{t("tools.providerNone", "未配置")}</option>
          {PROVIDERS.map((p) => (
            <option key={p} value={p}>
              {providerLabel(p, t)}
              {UNVERIFIED.has(p) ? ` ${t("tools.unverifiedBadge", "（未验证）")}` : ""}
            </option>
          ))}
        </select>
      </label>

      <label style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12 }}>
        {t("tools.enabled", "启用搜索")}
        <input
          type="checkbox"
          data-testid="mcp-tools-enabled"
          checked={enabled}
          onChange={(e) => { setEnabled(e.target.checked); setSaved(false); }}
        />
      </label>

      <label style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12 }}>
        <span>{t("tools.keys", "API key（每行一个，多 key 自动轮换）")}</span>
        <textarea
          data-testid="mcp-tools-keys"
          rows={3}
          value={keysText}
          onChange={(e) => { setKeysText(e.target.value); setSaved(false); }}
          style={{ width: "100%", boxSizing: "border-box" }}
        />
      </label>

      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 12, fontWeight: 600 }}>{t("tools.engines.title", "免 key 搜索引擎")}</div>
        <div className="text-secondary" style={{ fontSize: 12 }}>
          {t("tools.engines.desc", "未配置 key 服务时，并行查询已开启的引擎，结果按 URL 去重合并（最多 10 条）；单个引擎失败会被跳过。")}
        </div>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "6px 18px" }}>
          {SEARCH_ENGINES.map((e) => (
            <label key={e.id} style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12 }}>
              <input
                type="checkbox"
                data-testid={`tools-engine-${e.id}`}
                checked={engines[e.id] ?? e.defaultOn}
                onChange={(ev) => { setEngines({ ...engines, [e.id]: ev.target.checked }); setSaved(false); }}
              />
              {t(`tools.engines.${e.id}`, e.id)}
            </label>
          ))}
        </div>
      </div>

      <label style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12 }}>
        <span>{t("tools.searxngUrls", "SearXNG 实例 URL（每行一个，按序尝试；留空则该引擎被跳过）")}</span>
        <textarea
          data-testid="mcp-tools-searxng-urls"
          rows={2}
          value={urlsText}
          onChange={(e) => { setUrlsText(e.target.value); setSaved(false); }}
          style={{ width: "100%", boxSizing: "border-box" }}
        />
      </label>

      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <Button data-testid="tools-searx-refresh" variant="outline" disabled={searxBusy} onClick={() => void refreshSearx()}>
            {searxBusy ? t("tools.searxRefreshing", "刷新中…") : t("tools.searxRefresh", "刷新公共列表")}
          </Button>
          <span className="text-secondary" style={{ fontSize: 12 }}>
            {t("tools.searxRefreshHint", "从 searx.space 拉取一次健康实例供多选；仅在点击时联网，不会自动刷新。")}
          </span>
        </div>
        {searxError && <span style={{ color: "var(--danger)", fontSize: 12 }} data-testid="tools-searx-error">{searxError}</span>}
        {searxList.length > 0 && (
          <div style={{ display: "flex", flexDirection: "column", gap: 4, maxHeight: 180, overflowY: "auto" }}>
            {searxList.map((inst, i) => (
              <label key={inst.url} style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12 }}>
                <input
                  type="checkbox"
                  data-testid={`tools-searx-inst-${i}`}
                  checked={linesOf(urlsText).includes(inst.url)}
                  onChange={(ev) => toggleSearx(inst.url, ev.target.checked)}
                />
                {inst.url}
                {inst.seconds != null && <span className="text-secondary">{inst.seconds.toFixed(2)}s</span>}
              </label>
            ))}
          </div>
        )}
      </div>

      <div style={{ fontSize: 12, fontWeight: 600 }}>
        {t("tools.mediaTitle", "生图 provider（OpenAI images 兼容端点）")}
      </div>
      {mediaSection(t("tools.imageTitle", "生图（generate_image）"), "image", image, setImage)}

      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <Button data-testid="mcp-tools-save" onClick={() => void save()}>
          {t("tools.save", "保存")}
        </Button>
        {saved && <span style={{ color: "var(--success)", fontSize: 12 }} data-testid="mcp-tools-saved">{t("tools.saved", "已保存")}</span>}
        {saveError && <span style={{ color: "var(--danger)", fontSize: 12 }} data-testid="mcp-tools-save-error">{saveError}</span>}
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <Button data-testid="mcp-tools-install" disabled={installing} onClick={() => void install()}>
          {t("tools.install", "一键写入本机 agent 配置")}
        </Button>
        <span className="text-secondary" style={{ fontSize: 12 }}>
          {t(
            "tools.installHint",
            "把 AiDog 的 HTTP MCP 地址（http://127.0.0.1:<代理端口>/mcp，默认分组 key 作 Bearer）写进本机 Claude Code（~/.claude.json）与 Codex（~/.codex/config.toml）的 mcpServers；既有条目保留，同名 aidog 条目（含旧 stdio 版）被替换。",
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
