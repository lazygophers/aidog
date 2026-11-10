// ─── 「扩展 → 工具」页主体：方向 A 总控台（rail 状态总览 + 右侧单卡详情，每卡独立保存）──
// 设置键 scope=mcp_tools / key=settings。搜索卡读写票 02 新形状（sources 有序列表 /
// services[id].keys / mode），keys 路径保持 `keys` 结尾（管理接口脱敏按该后缀匹配，
// secret_fields.rs）；api_key / keys 一律密码框（默认隐藏），加载回填的 `***` 提交原样
// 保持原值。保存走 settingsApi（settings_set），SECRET_PATHS 脱敏 + RESTORERS 回填；
// 每卡保存前重新 settingsApi.get 只覆盖本卡字段（读-改-写，防两卡互相覆盖）。
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { settingsApi } from "../../services/api/settings";
import { mcpApi } from "../../services/api/mcp";
import { searchApi, type SearchSource, type SearxInstance } from "../../services/api/search";
import { registerNavGuard } from "../../utils/navGuard";
import { SortableList } from "../../components/SortableList";
import { Switch } from "../../components/ui/switch";
import { Button } from "../../components/ui/button";
import { Badge } from "../../components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { cn } from "../../lib/utils";

export type ToolId = "web_search" | "web_fetch" | "generate_image";
export type ToolState = "configured" | "unconfigured" | "disabled";
type CardId = ToolId | "install";

/** 生图 provider 配置（MediaProviderConfig，snake_case 与后端对齐）。 */
type MediaCfg = { base_url: string; api_key: string; model: string };
/** 搜索草稿（searxRaw = SearXNG 实例 URL 原文，保存时按行解析）。 */
type SearchDraft = {
  enabled: boolean;
  sources: string[];
  mode: "sequential" | "parallel";
  services: Record<string, { keys: string[] }>;
  searxRaw: string;
};

const EMPTY_MEDIA: MediaCfg = { base_url: "", api_key: "", model: "" };
const DEFAULT_SEARCH: SearchDraft = {
  enabled: true,
  sources: [],
  mode: "sequential",
  services: {},
  searxRaw: "",
};

const hasMedia = (c: MediaCfg): boolean =>
  c.base_url !== "" || c.api_key !== "" || c.model !== "";
const linesOf = (s: string): string[] =>
  s.split("\n").map((l) => l.trim()).filter((l) => l !== "");
const nonEmptyKey = (k: unknown): k is string => typeof k === "string" && k.trim() !== "";

const readMedia = (v: unknown): MediaCfg => {
  const m = (v ?? {}) as Record<string, unknown>;
  return {
    base_url: typeof m.base_url === "string" ? m.base_url : "",
    api_key: typeof m.api_key === "string" ? m.api_key : "",
    model: typeof m.model === "string" ? m.model : "",
  };
};

const readSearch = (v: unknown): SearchDraft => {
  const s = (v ?? {}) as Record<string, unknown>;
  const svc = (s.services ?? {}) as Record<string, { keys?: unknown }>;
  const services: SearchDraft["services"] = {};
  for (const [id, c] of Object.entries(svc)) {
    services[id] = { keys: Array.isArray(c?.keys) ? c.keys.filter(nonEmptyKey) : [] };
  }
  return {
    enabled: s.enabled !== false,
    sources: Array.isArray(s.sources) ? s.sources.filter((x): x is string => typeof x === "string") : [],
    mode: s.mode === "parallel" ? "parallel" : "sequential",
    services,
    searxRaw: Array.isArray(s.searxng_base_urls)
      ? s.searxng_base_urls.filter((x): x is string => typeof x === "string").join("\n")
      : "",
  };
};

/** 保存前清理：keys trim 去空、全空的 service 条目丢弃（`***` 非空保留）。 */
const cleanSearch = (d: SearchDraft): Record<string, unknown> => {
  const services: Record<string, { keys: string[] }> = {};
  for (const [id, c] of Object.entries(d.services)) {
    const keys = c.keys.filter((k) => k.trim() !== "").map((k) => k.trim());
    if (keys.length > 0) services[id] = { keys };
  }
  return {
    enabled: d.enabled,
    sources: d.sources,
    mode: d.mode,
    services,
    searxng_base_urls: linesOf(d.searxRaw),
  };
};

/**
 * 工具状态判定（纯函数，rail 三态）。可用来源 = 免 key 引擎，或 key 服务且
 * `services[id].keys` 至少一个非空（脱敏 `***` 视为非空）。keyedIds = key 服务 id 集，
 * 由 `searchApi.sources()` 的 kind 派生（缺省空集 = 全部按免 key 处理）。
 */
export function toolStatus(
  tool: ToolId,
  settings: unknown,
  keyedIds: ReadonlySet<string> = new Set(),
): ToolState {
  const s = (settings ?? {}) as Record<string, any>;
  if (tool === "web_search") {
    if (s.search?.enabled === false) return "disabled";
    const sources: string[] = Array.isArray(s.search?.sources) ? s.search.sources : [];
    const services = (s.search?.services ?? {}) as Record<string, { keys?: string[] }>;
    const usable = sources.some(
      (id) => !keyedIds.has(id) || (services[id]?.keys ?? []).some((k) => k && k.trim() !== ""),
    );
    return usable ? "configured" : "unconfigured";
  }
  if (tool === "web_fetch") {
    return s.fetch?.enabled === false ? "disabled" : "configured";
  }
  const img = s.image as Partial<MediaCfg> | null | undefined;
  if (!img) return "unconfigured";
  return img.base_url?.trim() && img.api_key?.trim() && img.model?.trim() ? "configured" : "unconfigured";
}

// ─── 小图标（内联 SVG，与 demo P.grip / P.eye 同形）───
const GripIcon = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
    <circle cx="6" cy="4" r=".9" /><circle cx="10" cy="4" r=".9" />
    <circle cx="6" cy="8" r=".9" /><circle cx="10" cy="8" r=".9" />
    <circle cx="6" cy="12" r=".9" /><circle cx="10" cy="12" r=".9" />
  </svg>
);
const EyeIcon = () => (
  <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z" />
    <circle cx="8" cy="8" r="2" />
  </svg>
);
const XIcon = () => (
  <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" aria-hidden="true">
    <path d="M4 4l8 8M12 4l-8 8" />
  </svg>
);

/** 密码框（默认 type=password）+ 显隐眼睛按钮。eye testid = `${inputTestId}-eye`。 */
function SecretInput({
  inputTestId,
  value,
  onChange,
  revealed,
  onToggle,
  placeholder,
}: {
  inputTestId: string;
  value: string;
  onChange: (v: string) => void;
  revealed: boolean;
  onToggle: () => void;
  placeholder?: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="relative flex">
      <input
        type={revealed ? "text" : "password"}
        data-testid={inputTestId}
        value={value}
        placeholder={placeholder}
        autoComplete="off"
        onChange={(e) => onChange(e.target.value)}
        className="h-8 w-full rounded-md border border-input bg-card px-2.5 pr-9 font-mono text-sm outline-none transition-colors focus:border-primary"
      />
      <button
        type="button"
        data-testid={`${inputTestId}-eye`}
        aria-label={revealed ? t("tools.key.hide") : t("tools.key.show")}
        onClick={onToggle}
        className="absolute right-1.5 top-1/2 -translate-y-1/2 grid size-6 place-items-center rounded text-secondary transition-colors hover:bg-[var(--accent-subtle)] hover:text-primary"
      >
        <EyeIcon />
      </button>
    </div>
  );
}

/** 状态 pill（rail / 卡头共用）。 */
function StatusPill({ tool, state }: { tool: ToolId; state: ToolState }) {
  const { t } = useTranslation();
  return (
    <Badge
      variant="outline"
      data-testid={`tools-status-${tool}`}
      data-state={state}
      className={cn(
        "gap-1.5 border-transparent font-medium",
        state === "configured" && "bg-[var(--accent-subtle)] text-[var(--success)]",
        state === "unconfigured" && "bg-secondary text-secondary",
        state === "disabled" && "bg-[var(--color-warning-bg)] text-[var(--warning)]",
      )}
    >
      <span className="size-1.5 rounded-full bg-current" />
      {t(`tools.status.${state}`)}
    </Badge>
  );
}

/** 执行路径图节点。 */
function FlowNode({ title, sub, io }: { title: string; sub?: string; io?: boolean }) {
  return (
    <span
      className={cn(
        "rounded-md border px-2.5 py-1.5 text-xs font-semibold leading-tight whitespace-nowrap",
        io ? "border-input bg-secondary" : "border-input bg-card",
      )}
    >
      {title}
      {sub && <span className="block text-xs font-normal text-secondary">{sub}</span>}
    </span>
  );
}
function FlowArrow({ label, tone }: { label?: string; tone?: "ok" | "info" }) {
  return (
    <span className={cn("relative flex min-w-[30px] items-center justify-center gap-1 px-1", label && "min-w-[96px]")}>
      <span className="h-px min-w-[12px] flex-1 border-t border-dashed border-[var(--input)]" />
      <span className={cn("size-1.5 rotate-45 border-t border-r", tone === "ok" ? "border-[var(--success)]" : tone === "info" ? "border-primary" : "border-[var(--muted-foreground)]")} />
      {label && (
        <span className={cn("absolute -top-2 left-1/2 -translate-x-1/2 whitespace-nowrap text-xs", tone === "ok" ? "text-[var(--success)]" : tone === "info" ? "text-primary" : "text-destructive")}>
          {label}
        </span>
      )}
    </span>
  );
}

export function ToolsSection() {
  const { t } = useTranslation();
  const [loadError, setLoadError] = useState("");
  const [search, setSearch] = useState<SearchDraft>(DEFAULT_SEARCH);
  const [fetchOn, setFetchOn] = useState(true);
  const [image, setImage] = useState<MediaCfg>(EMPTY_MEDIA);
  const [snap, setSnap] = useState(() => ({
    search: JSON.stringify(DEFAULT_SEARCH),
    fetch: JSON.stringify(true),
    image: JSON.stringify(EMPTY_MEDIA),
  }));
  const [sourceList, setSourceList] = useState<SearchSource[]>([]);
  const [selected, setSelected] = useState<CardId>("web_search");
  const [pendingNav, setPendingNav] = useState<{ proceed: () => void } | null>(null);
  const [savedCard, setSavedCard] = useState<"search" | "fetch" | "image" | "">("");
  // 最近一次持久化的三卡值：rail 状态按已存配置算，草稿改动不影响状态显示
  const [saved, setSaved] = useState(() => ({ search: DEFAULT_SEARCH, fetch: true, image: EMPTY_MEDIA }));
  const [saveError, setSaveError] = useState("");
  const [revealed, setRevealed] = useState<ReadonlySet<string>>(new Set());
  // SearXNG 公共实例（只在点按钮时拉一次）
  const [searxList, setSearxList] = useState<SearxInstance[]>([]);
  const [searxBusy, setSearxBusy] = useState(false);
  const [searxError, setSearxError] = useState("");
  const [searxAdd, setSearxAdd] = useState("");
  // 行集合会话内累积（known）：已启用 URL ∪ 刷新拉到的公共实例 ∪ 手动添加，去重保序；
  // 「全不选」清空启用集后行仍显示（known 不丢），删行才从 known 移除
  const [searxKnown, setSearxKnown] = useState<string[]>([]);
  useEffect(() => {
    setSearxKnown((prev) => {
      const seen = new Set(prev);
      const rows = [...prev];
      for (const u of [...linesOf(search.searxRaw), ...searxList.map((x) => x.url)]) {
        if (!seen.has(u)) {
          seen.add(u);
          rows.push(u);
        }
      }
      return rows.length === prev.length ? prev : rows;
    });
  }, [search.searxRaw, searxList]);
  const enabledUrls = useMemo(() => new Set(linesOf(search.searxRaw)), [search.searxRaw]);
  const [installing, setInstalling] = useState(false);
  const [installMsg, setInstallMsg] = useState("");
  const [installOk, setInstallOk] = useState(false);

  const load = useCallback(async () => {
    try {
      const v = ((await settingsApi.get("mcp_tools", "settings")) ?? {}) as Record<string, unknown>;
      const s = readSearch(v.search);
      const f = (v.fetch ?? {}) as { enabled?: unknown };
      const fOn = typeof f.enabled === "boolean" ? f.enabled : true;
      const img = readMedia(v.image);
      setSearch(s);
      setFetchOn(fOn);
      setImage(img);
      setSaved({ search: s, fetch: fOn, image: img });
      setSnap({
        search: JSON.stringify(s),
        fetch: JSON.stringify(fOn),
        image: JSON.stringify(img),
      });
      setSavedCard("");
      setSaveError("");
      setLoadError("");
    } catch (e) {
      setLoadError(String(e));
    }
  }, []);
  useEffect(() => { void load(); }, [load]);

  // 来源清单（后端唯一真值源 search_sources）
  useEffect(() => {
    let alive = true;
    searchApi.sources().then((l) => { if (alive) setSourceList(l); }).catch(() => {});
    return () => { alive = false; };
  }, []);

  const keyedIds = useMemo(
    () => new Set(sourceList.filter((s) => s.kind === "keyed").map((s) => s.id)),
    [sourceList],
  );
  const sourceName = useCallback(
    (id: string) => t(`tools.source.${id}`, id),
    [t],
  );

  const dirty = {
    search: JSON.stringify(search) !== snap.search,
    fetch: JSON.stringify(fetchOn) !== snap.fetch,
    image: JSON.stringify(image) !== snap.image,
  };
  const unsavedCount = Number(dirty.search) + Number(dirty.fetch) + Number(dirty.image);

  // 未保存离开拦截：页面级一个 dirty 汇总，挂载期间只此一个 guard（last wins）
  useEffect(() => {
    if (unsavedCount === 0) return;
    return registerNavGuard((proceed) => setPendingNav({ proceed }));
  }, [unsavedCount]);

  const discardAndLeave = () => {
    const proceed = pendingNav?.proceed;
    setPendingNav(null);
    void load();
    proceed?.(); // 先放行导航（同步），草稿还原由 load 异步完成
  };

  // 每卡保存：保存前重新 get，只覆盖本卡字段（读-改-写）
  const saveCard = async (card: "search" | "fetch" | "image") => {
    setSavedCard("");
    setSaveError("");
    try {
      const cur = ((await settingsApi.get("mcp_tools", "settings")) ?? {}) as Record<string, unknown>;
      const next: Record<string, unknown> = { ...cur };
      let savedValue: unknown;
      if (card === "search") {
        next.search = cleanSearch(search);
        savedValue = search;
      } else if (card === "fetch") {
        next.fetch = { enabled: fetchOn };
        savedValue = fetchOn;
      } else {
        if (hasMedia(image)) next.image = image;
        else delete next.image;
        savedValue = image;
      }
      await settingsApi.set("mcp_tools", "settings", next);
      setSnap((prev) => ({ ...prev, [card]: JSON.stringify(savedValue) }));
      setSaved((prev) => ({ ...prev, [card]: savedValue }) as typeof prev);
      setSavedCard(card);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  // ── 搜索卡操作 ──
  const toggleSource = (id: string) => {
    setSearch((s) => {
      if (s.sources.includes(id)) {
        return { ...s, sources: s.sources.filter((x) => x !== id) };
      }
      const keyed = keyedIds.has(id);
      return {
        ...s,
        sources: [...s.sources, id],
        services: keyed && !s.services[id]
          ? { ...s.services, [id]: { keys: [""] } }
          : s.services,
      };
    });
  };
  const removeSource = (id: string) => {
    setSearch((s) => ({ ...s, sources: s.sources.filter((x) => x !== id) }));
  };
  const setMode = (mode: "sequential" | "parallel") => setSearch((s) => ({ ...s, mode }));
  const keysOf = (id: string): string[] => search.services[id]?.keys ?? [];
  const setKeys = (id: string, keys: string[]) =>
    setSearch((s) => ({ ...s, services: { ...s.services, [id]: { keys } } }));
  const addKey = (id: string) => setKeys(id, [...keysOf(id), ""]);
  const delKey = (id: string, i: number) => {
    const keys = keysOf(id).filter((_, j) => j !== i);
    setKeys(id, keys.length > 0 ? keys : [""]);
  };
  const toggleReveal = (keyId: string) =>
    setRevealed((prev) => {
      const next = new Set(prev);
      if (next.has(keyId)) next.delete(keyId);
      else next.add(keyId);
      return next;
    });
  const setUrlsRaw = (raw: string) => setSearch((s) => ({ ...s, searxRaw: raw }));
  const toggleSearxUrl = (url: string) => {
    const lines = linesOf(search.searxRaw);
    setUrlsRaw((lines.includes(url) ? lines.filter((l) => l !== url) : [...lines, url]).join("\n"));
  };
  // 行内测试：耗时缓存仅本会话内存（searxTest），不持久化
  const [searxTest, setSearxTest] = useState<Record<string, { testing: boolean; seconds: number | null; fail: boolean }>>({});

  const testSearx = async (url: string) => {
    setSearxTest((m) => ({ ...m, [url]: { testing: true, seconds: null, fail: false } }));
    try {
      const seconds = await searchApi.searxTest(url);
      setSearxTest((m) => ({ ...m, [url]: { testing: false, seconds, fail: false } }));
    } catch {
      setSearxTest((m) => ({ ...m, [url]: { testing: false, seconds: null, fail: true } }));
    }
  };
  const searxBatch = (urls: string[]) => setUrlsRaw(urls.join("\n"));
  const searxAddUrl = () => {
    const u = searxAdd.trim();
    if (u === "") return;
    setUrlsRaw([...new Set([...linesOf(search.searxRaw), u])].join("\n"));
    setSearxAdd("");
  };
  const searxDelRow = (url: string) => {
    setUrlsRaw(linesOf(search.searxRaw).filter((l) => l !== url).join("\n"));
    setSearxList((l) => l.filter((x) => x.url !== url));
    setSearxKnown((k) => k.filter((x) => x !== url));
    setSearxTest((m) => {
      if (!(url in m)) return m;
      const n = { ...m };
      delete n[url];
      return n;
    });
  };

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

  // ── 执行路径图（随模式 / 停用切换）与 rail 状态（按已存值）──
  const liveSettings = useMemo(
    () => ({ search: saved.search, fetch: { enabled: saved.fetch }, image: saved.image }),
    [saved],
  );
  const statusOf = (tool: ToolId) => toolStatus(tool, liveSettings, keyedIds);

  const flow = (() => {
    if (!search.enabled || search.sources.length === 0) {
      return (
        <div
          data-testid="tools-flow"
          data-state="off"
          className="flex items-center justify-center rounded-lg border border-dashed border-border px-4 py-5 text-xs text-secondary"
        >
          {t("tools.flow.off")}
        </div>
      );
    }
    const srcNode = (id: string) => (
      <FlowNode title={sourceName(id)} sub={keyedIds.has(id) ? t("tools.flow.sources") : undefined} />
    );
    const seq = (
      <div data-testid="tools-flow" data-state="sequential" className="flex flex-wrap items-center gap-x-0 gap-y-2 rounded-lg border border-border bg-card p-4">
        <FlowNode title={t("tools.flow.mcp")} sub={t("tools.flow.gateway")} io />
        <FlowArrow />
        <FlowNode title="web_search" />
        {search.sources.map((id, i) => (
          <span key={id} className="contents">
            <FlowArrow label={i > 0 ? t("tools.flow.failNext") : undefined} />
            <span className="tools-flow-src">{srcNode(id)}</span>
          </span>
        ))}
        <FlowArrow label={t("tools.flow.okReturn")} tone="ok" />
        <FlowNode title={t("tools.flow.log")} io />
      </div>
    );
    const par = (
      <div data-testid="tools-flow" data-state="parallel" className="tools-flow-par flex flex-wrap items-center gap-x-0 gap-y-2 rounded-lg border border-border bg-card p-4">
        <FlowNode title={t("tools.flow.mcp")} sub={t("tools.flow.gateway")} io />
        <FlowArrow label={t("tools.flow.concurrent")} tone="info" />
        <span className="tools-flow-src flex flex-col gap-2 rounded-lg border-x border-dashed border-[var(--input)] px-4 py-2">
          {search.sources.map((id) => <span key={id}>{srcNode(id)}</span>)}
        </span>
        <FlowArrow />
        <FlowNode title={t("tools.flow.merge")} />
        <FlowArrow />
        <FlowNode title={t("tools.flow.log")} io />
      </div>
    );
    return search.mode === "sequential" ? seq : par;
  })();

  // ── 渲染块 ──
  const railItems: { id: CardId; title: string; summary: string; tool?: ToolId }[] = [
    {
      id: "web_search",
      title: t("tools.card.search"),
      tool: "web_search",
      summary: statusOf("web_search") === "disabled" ? t("tools.disabled.search") : "",
    },
    {
      id: "web_fetch",
      title: t("tools.card.fetch"),
      tool: "web_fetch",
      summary: statusOf("web_fetch") === "disabled" ? t("tools.disabled.fetch") : "",
    },
    { id: "generate_image", title: t("tools.card.image"), tool: "generate_image", summary: "" },
    { id: "install", title: t("tools.card.install"), summary: t("tools.installHint").slice(0, 24) + "…" },
  ];

  const cardFooter = (card: "search" | "fetch" | "image", saveTestId: string) => (
    <div className="mt-4 flex items-center gap-3 rounded-b-xl border-t border-border bg-secondary px-4 py-2.5">
      {dirty[card] && (
        <span
          data-testid={`tools-dirty-${card}`}
          className="flex items-center gap-1.5 text-xs text-[var(--warning)]"
        >
          <span className="size-1.5 rounded-full bg-current" />
          {t("tools.unsaved.badge")}
        </span>
      )}
      <span className="flex-1" />
      {savedCard === card && !dirty[card] && (
        <span data-testid="mcp-tools-saved" className="text-xs text-[var(--success)]">{t("tools.saved")}</span>
      )}
      {saveError && (
        <span data-testid="mcp-tools-save-error" className="text-xs text-destructive">{saveError}</span>
      )}
      <Button size="sm" data-testid={saveTestId} onClick={() => void saveCard(card)}>
        {t("tools.saveCard")}
      </Button>
    </div>
  );

  const cardHeader = (title: string, extra?: ReactNode) => (
    <div className="flex items-center gap-2.5 border-b border-border px-4 py-3">
      <h3 className="text-sm font-semibold">{title}</h3>
      <span className="flex-1" />
      {extra}
    </div>
  );

  const switchRow = (testid: string, checked: boolean, onToggle: () => void, label: string) => (
    <label className="flex items-center gap-2.5 text-sm">
      <Switch data-testid={testid} checked={checked} onCheckedChange={onToggle} />
      {label}
    </label>
  );

  const poolGroup = (title: string, items: SearchSource[]) => (
    <div className="mb-3.5">
      <div className="mb-1.5 text-sm font-semibold text-secondary">{title}</div>
      <div className="flex flex-wrap gap-1.5">
        {items.map((src) => {
          const on = search.sources.includes(src.id);
          return (
            <button
              key={src.id}
              type="button"
              data-testid={`tools-src-chip-${src.id}`}
              data-state={on ? "on" : "off"}
              onClick={() => toggleSource(src.id)}
              className={cn(
                "inline-flex h-7 items-center gap-1.5 rounded-md border px-2.5 text-sm transition-colors",
                on
                  ? "border-primary bg-[var(--accent-subtle)] font-medium text-primary"
                  : "border-border bg-card hover:border-primary",
              )}
            >
              <span className={cn("grid size-3 place-items-center rounded-[3px] border", on ? "border-primary bg-primary" : "border-[var(--input)]")} />
              {sourceName(src.id)}
            </button>
          );
        })}
      </div>
    </div>
  );

  const keyRows = (id: string) => (
    <div className="flex flex-col gap-1.5 px-2.5 pb-2.5 pl-[52px]">
      {keysOf(id).map((k, i) => (
        <div key={i} data-testid={`tools-key-row-${id}-${i}`} className="flex items-center gap-1.5">
          <SecretInput
            inputTestId={`tools-key-input-${id}-${i}`}
            value={k}
            onChange={(v) => setKeys(id, keysOf(id).map((x, j) => (j === i ? v : x)))}
            revealed={revealed.has(`tools-key-input-${id}-${i}`)}
            onToggle={() => toggleReveal(`tools-key-input-${id}-${i}`)}
            placeholder="API key"
          />
          <button
            type="button"
            data-testid={`tools-key-del-${id}-${i}`}
            aria-label={`${id} key ${i + 1} ✕`}
            onClick={() => delKey(id, i)}
            className="grid size-6 place-items-center rounded text-secondary transition-colors hover:bg-[color-mix(in_srgb,var(--danger)_14%,transparent)] hover:text-destructive"
          >
            <XIcon />
          </button>
        </div>
      ))}
      <div>
        <Button variant="ghost" size="sm" data-testid={`tools-key-add-${id}`} onClick={() => addKey(id)}>
          {t("tools.key.add")}
        </Button>
      </div>
    </div>
  );

  const searxRow = (
    <div className="flex flex-col gap-1.5 px-2.5 pb-2.5 pl-[52px]">
      <div className="text-sm font-medium text-secondary">{t("tools.searxngUrls")}</div>
      <div className="flex flex-wrap items-center gap-1.5">
        <Button variant="outline" size="sm" data-testid="tools-searx-all" onClick={() => searxBatch(searxKnown)}>{t("tools.searxAll")}</Button>
        <Button variant="outline" size="sm" data-testid="tools-searx-invert" onClick={() => searxBatch(searxKnown.filter((u) => !enabledUrls.has(u)))}>{t("tools.searxInvert")}</Button>
        <Button variant="outline" size="sm" data-testid="tools-searx-none" onClick={() => searxBatch([])}>{t("tools.searxNone")}</Button>
        <Button variant="outline" size="sm" data-testid="tools-searx-available" onClick={() => searxBatch(searxKnown.filter((u) => searxTest[u] && !searxTest[u].fail && searxTest[u].seconds != null))}>{t("tools.searxAvailable")}</Button>
        <span className="flex-1" />
        <Button variant="outline" size="sm" data-testid="tools-searx-refresh" disabled={searxBusy} onClick={() => void refreshSearx()}>
          {searxBusy ? t("tools.searxRefreshing") : t("tools.searxRefresh")}
        </Button>
      </div>
      {searxError && (
        <span className="text-sm text-destructive" data-testid="tools-searx-error">{searxError}</span>
      )}
      <div className="flex max-h-[220px] flex-col gap-1 overflow-y-auto">
        {searxKnown.map((url, i) => {
          const on = enabledUrls.has(url);
          const st = searxTest[url];
          const listSec = searxList.find((x) => x.url === url)?.seconds ?? null;
          const seconds = st?.fail || st?.testing ? null : (st?.seconds ?? listSec);
          return (
            <div
              key={url}
              data-testid={`tools-searx-row-${i}`}
              data-state={on ? "on" : "off"}
              onClick={() => toggleSearxUrl(url)}
              className={cn(
                "flex h-8 cursor-pointer items-center gap-2 rounded-md border px-2.5 text-sm transition-colors select-none",
                on ? "border-primary bg-[var(--accent-subtle)] text-primary" : "border-border bg-card hover:border-primary",
              )}
            >
              <span className={cn("grid size-3 shrink-0 place-items-center rounded-[3px] border", on ? "border-primary bg-primary" : "border-[var(--input)]")} />
              <span className="min-w-0 flex-1 truncate font-mono text-sm">{url}</span>
              {st?.testing ? (
                <span data-testid={`tools-searx-sec-${i}`} className="shrink-0 text-xs text-secondary">{t("tools.searxTesting")}</span>
              ) : (
                <span
                  data-testid={`tools-searx-sec-${i}`}
                  className={cn("shrink-0 font-mono text-xs", st?.fail ? "text-destructive" : "text-secondary")}
                >
                  {st?.fail ? t("tools.searxFail") : seconds != null ? `${seconds.toFixed(2)}s` : ""}
                </span>
              )}
              {st?.testing ? (
                <Button variant="ghost" size="sm" disabled data-testid={`tools-searx-test-${i}`}>{t("tools.searxTesting")}</Button>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  data-testid={`tools-searx-test-${i}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    void testSearx(url);
                  }}
                >
                  {t("tools.searxTest")}
                </Button>
              )}
              <button
                type="button"
                data-testid={`tools-searx-del-${i}`}
                aria-label={`${url} ✕`}
                onClick={(e) => {
                  e.stopPropagation();
                  searxDelRow(url);
                }}
                className="grid size-6 shrink-0 place-items-center rounded text-secondary transition-colors hover:bg-[color-mix(in_srgb,var(--danger)_14%,transparent)] hover:text-destructive"
              >
                <XIcon />
              </button>
            </div>
          );
        })}
      </div>
      <div className="flex items-center gap-1.5">
        <input
          data-testid="tools-searx-add"
          value={searxAdd}
          placeholder="https://…"
          onChange={(e) => setSearxAdd(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") searxAddUrl();
          }}
          className="h-8 flex-1 rounded-md border border-input bg-card px-2.5 font-mono text-sm outline-none transition-colors focus:border-primary"
        />
        <Button variant="outline" size="sm" data-testid="tools-searx-add-btn" onClick={searxAddUrl}>{t("tools.searxAdd")}</Button>
      </div>
    </div>
  );

  const searchCard = (
    <section data-testid="mcp-tools-card-search" className="rounded-xl border border-border bg-card shadow-sm">
      {cardHeader(
        t("tools.card.search"),
        switchRow("mcp-tools-enabled", search.enabled, () => setSearch((s) => ({ ...s, enabled: !s.enabled })), t("tools.enabled")),
      )}
      <div className="px-4 pb-4 pt-4">
        <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
          <div>
            <div className="mb-2 flex items-center gap-2 text-sm font-semibold">
              {t("tools.sources.pool")}
            </div>
            {poolGroup(t("tools.sources.keyed"), sourceList.filter((s) => s.kind === "keyed"))}
            {poolGroup(t("tools.sources.engines"), sourceList.filter((s) => s.kind === "engine"))}
          </div>
          <div>
            <div className="mb-2 text-sm font-semibold text-secondary">{t("tools.sources.enabled")}</div>
            {search.sources.length === 0 ? (
              <div className="rounded-lg border border-dashed border-[var(--input)] px-4 py-4 text-center text-sm text-secondary">
                {t("tools.sources.empty")}
              </div>
            ) : (
              <SortableList
                items={search.sources.map((id) => ({ id }))}
                onReorder={(next) => setSearch((s) => ({ ...s, sources: next.map((x) => x.id) }))}
                renderItem={(item, handle) => (
                  <div className="rounded-lg border border-border bg-card transition-shadow">
                    <div className="flex items-center gap-2 px-2.5 py-1.5">
                      <button
                        type="button"
                        data-testid={`tools-src-grip-${item.id}`}
                        aria-label={t("tools.sources.dragHandle")}
                        className="cursor-grab touch-none text-secondary active:cursor-grabbing"
                        ref={handle.ref}
                        {...handle.attributes}
                        {...handle.listeners}
                      >
                        <GripIcon />
                      </button>
                      <span className="grid size-5 place-items-center rounded bg-primary text-xs font-semibold text-primary-foreground">
                        {search.sources.indexOf(item.id) + 1}
                      </span>
                      <b className="text-sm font-semibold">{sourceName(item.id)}</b>
                      {keyedIds.has(item.id) && (
                        <span className="rounded bg-[var(--accent-subtle)] px-1 text-xs text-primary">key</span>
                      )}
                      <span className="flex-1" />
                      <button
                        type="button"
                        data-testid={`tools-src-remove-${item.id}`}
                        aria-label={sourceName(item.id) + " ✕"}
                        onClick={() => removeSource(item.id)}
                        className="grid size-6 place-items-center rounded text-secondary transition-colors hover:bg-[color-mix(in_srgb,var(--danger)_14%,transparent)] hover:text-destructive"
                      >
                        <XIcon />
                      </button>
                    </div>
                    {keyedIds.has(item.id) && keyRows(item.id)}
                    {item.id === "searxng" && searxRow}
                  </div>
                )}
              />
            )}
          </div>
        </div>

        <div className="mt-3.5 flex items-center gap-3">
          <div className="text-sm font-semibold">{t("tools.mode.label")}</div>
          <div className="inline-flex rounded-lg border border-input bg-secondary p-0.5">
            <button
              type="button"
              data-testid="tools-mode-sequential"
              data-state={search.mode === "sequential" ? "active" : "inactive"}
              onClick={() => setMode("sequential")}
              className={cn(
                "h-7 rounded-md px-3 text-sm font-medium transition-colors",
                search.mode === "sequential" ? "bg-card text-primary shadow-sm" : "text-secondary",
              )}
            >
              {t("tools.mode.sequential")}
            </button>
            <button
              type="button"
              data-testid="tools-mode-parallel"
              data-state={search.mode === "parallel" ? "active" : "inactive"}
              onClick={() => setMode("parallel")}
              className={cn(
                "h-7 rounded-md px-3 text-sm font-medium transition-colors",
                search.mode === "parallel" ? "bg-card text-primary shadow-sm" : "text-secondary",
              )}
            >
              {t("tools.mode.parallel")}
            </button>
          </div>
          <span className="text-sm text-secondary">
            {search.mode === "sequential" ? t("tools.mode.sequentialHint") : t("tools.mode.parallelHint")}
          </span>
        </div>

        {search.mode === "parallel" && search.sources.some((id) => keyedIds.has(id)) && (
          <div data-testid="tools-billing-warning" className="mt-2 flex items-center gap-1.5 text-sm text-[var(--warning)]">
            <span className="size-1.5 rounded-full bg-current" />
            {t("tools.billingWarning")}
          </div>
        )}

        <div className="mt-3.5">{flow}</div>
      </div>
      {cardFooter("search", "mcp-tools-save")}
    </section>
  );

  const fetchCard = (
    <section data-testid="mcp-tools-card-fetch" className="rounded-xl border border-border bg-card shadow-sm">
      {cardHeader(
        t("tools.card.fetch"),
        switchRow("mcp-tools-fetch-enabled", fetchOn, () => setFetchOn((v) => !v), t("tools.fetch.enabled")),
      )}
      <div className="px-4 pb-4 pt-4">
        <p className="text-xs text-secondary">{t("tools.fetch.desc")}</p>
      </div>
      {cardFooter("fetch", "tools-fetch-save")}
    </section>
  );

  const imageCard = (
    <section data-testid="mcp-tools-card-image" className="rounded-xl border border-border bg-card shadow-sm">
      {cardHeader(t("tools.card.image"))}
      <div className="flex flex-col gap-3 px-4 pb-4 pt-4">
        <label className="flex items-center gap-2.5 text-xs">
          base_url
          <input
            data-testid="mcp-tools-image-base-url"
            value={image.base_url}
            placeholder="https://api.openai.com/v1"
            onChange={(e) => setImage((c) => ({ ...c, base_url: e.target.value }))}
            className="h-8 flex-1 rounded-md border border-input bg-card px-2.5 font-mono text-sm outline-none transition-colors focus:border-primary"
          />
        </label>
        <label className="flex items-center gap-2.5 text-xs">
          {t("tools.apiKey")}
          <div className="flex-1">
            <SecretInput
              inputTestId="mcp-tools-image-api-key"
              value={image.api_key}
              onChange={(v) => setImage((c) => ({ ...c, api_key: v }))}
              revealed={revealed.has("mcp-tools-image-api-key")}
              onToggle={() => toggleReveal("mcp-tools-image-api-key")}
              placeholder="sk-…"
            />
          </div>
        </label>
        <label className="flex items-center gap-2.5 text-xs">
          {t("tools.model")}
          <input
            data-testid="mcp-tools-image-model"
            value={image.model}
            onChange={(e) => setImage((c) => ({ ...c, model: e.target.value }))}
            className="h-8 flex-1 rounded-md border border-input bg-card px-2.5 font-mono text-sm outline-none transition-colors focus:border-primary"
          />
        </label>
      </div>
      {cardFooter("image", "tools-image-save")}
    </section>
  );

  const installCard = (
    <section data-testid="mcp-tools-card-install" className="rounded-xl border border-border bg-card shadow-sm">
      {cardHeader(t("tools.card.install"))}
      <div className="flex items-center gap-3.5 px-4 py-4">
        <div className="min-w-0 flex-1">
          <div className="text-sm">{t("tools.installHint")}</div>
          <div className="mt-1 text-xs text-secondary">
            {t("tools.install")} · <code className="rounded bg-secondary px-1 font-mono text-xs">~/.claude.json</code>{" "}
            <code className="rounded bg-secondary px-1 font-mono text-xs">~/.codex/config.toml</code>
          </div>
        </div>
        <Button data-testid="mcp-tools-install" disabled={installing} onClick={() => void install()}>
          {t("tools.install")}
        </Button>
      </div>
      {installMsg && (
        <div
          data-testid="mcp-tools-install-result"
          className={cn("px-4 pb-4 text-xs", installOk ? "text-[var(--success)]" : "text-destructive")}
        >
          {installMsg}
        </div>
      )}
    </section>
  );

  const detailCard =
    selected === "web_search" ? searchCard
    : selected === "web_fetch" ? fetchCard
    : selected === "generate_image" ? imageCard
    : installCard;

  return (
    <div data-testid="mcp-tools-section" className="flex w-full flex-col gap-4">
      <div>
        <div className="section-title">{t("tools.title", "工具")}</div>
        <div className="mt-1 text-xs text-secondary">{t("tools.desc")}</div>
      </div>

      {loadError && (
        <div className="text-xs text-destructive" data-testid="mcp-tools-load-error">{loadError}</div>
      )}

      {unsavedCount > 0 && (
        <div
          data-testid="tools-unsaved-banner"
          className="flex items-center gap-1.5 rounded-lg border border-[color-mix(in_srgb,var(--warning)_30%,transparent)] bg-[var(--color-warning-bg)] px-3 py-2 text-xs text-[var(--warning)]"
        >
          <span className="size-1.5 rounded-full bg-current" />
          {t("tools.unsaved.banner", { count: unsavedCount })}
        </div>
      )}

      <div className="grid grid-cols-1 items-start gap-5 lg:grid-cols-[268px_1fr]">
        <aside className="flex flex-col gap-1.5">
          <div className="mb-0.5 px-0.5 font-mono text-xs text-secondary">{t("tools.nav.title")}</div>
          {railItems.map((item) => {
            const active = selected === item.id;
            return (
              <button
                key={item.id}
                type="button"
                data-testid={`tools-rail-${item.id}`}
                data-state={item.tool ? statusOf(item.tool) : undefined}
                onClick={() => setSelected(item.id)}
                className={cn(
                  "flex items-start gap-2.5 rounded-lg border px-3 py-2.5 text-left transition-all",
                  active
                    ? "border-primary bg-[var(--accent-subtle)] shadow-[inset_3px_0_0_var(--primary)]"
                    : "border-border bg-card hover:translate-x-0.5 hover:border-primary hover:bg-[var(--accent-subtle)]",
                )}
              >
                <span className="min-w-0 flex-1">
                  <b className="block text-sm font-semibold">{item.title}</b>
                  {item.summary && <small className="block text-xs text-secondary">{item.summary}</small>}
                </span>
                {item.tool && <StatusPill tool={item.tool} state={statusOf(item.tool)} />}
              </button>
            );
          })}
        </aside>
        <div className="min-w-0">{detailCard}</div>
      </div>

      <Dialog open={pendingNav !== null} onOpenChange={(o) => { if (!o) setPendingNav(null); }}>
        <DialogContent data-testid="tools-leave-dialog" className="max-w-sm">
          <DialogHeader>
            <DialogTitle>{t("tools.unsaved.leaveTitle")}</DialogTitle>
            <DialogDescription>{t("tools.unsaved.leaveBody", { count: unsavedCount })}</DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-2">
            <Button variant="outline" data-testid="tools-leave-cancel" onClick={() => setPendingNav(null)}>
              {t("tools.unsaved.leaveCancel")}
            </Button>
            <Button data-testid="tools-leave-confirm" onClick={discardAndLeave}>
              {t("tools.unsaved.leaveConfirm")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
