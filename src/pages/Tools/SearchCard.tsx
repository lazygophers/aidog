// 搜索卡（票 57-03 自 ToolsSection 拆出）：展示 + 客户端草稿变换。
// 草稿 / searx 测速等状态由容器经 props 传入（rail 切换不丢，行为与拆分前一致）；
// JSX 与拆分前逐字一致（props 沿用原变量名解构，切片不改写）。
import { useTranslation } from "react-i18next";
import type { SearchSource, SearxInstance } from "../../services/api/search";
import { SortableList } from "../../components/SortableList";
import { Button } from "../../components/ui/button";
import { cn } from "../../lib/utils";
import { isImeComposing } from "../../utils/ime";
import type { ReactNode } from "react";
import { CardFooter, CardHeader, SwitchRow } from "./CardChrome";
import type { ToolsCard, ToolsCardId } from "./useToolsCard";

/** 搜索草稿（searxRaw = SearXNG 实例 URL 原文，保存时按行解析）。 */
export type SearchDraft = {
  enabled: boolean;
  sources: string[];
  mode: "sequential" | "parallel";
  services: Record<string, { keys: string[] }>;
  searxRaw: string;
};


export const linesOf = (s: string): string[] =>
  s.split("\n").map((l) => l.trim()).filter((l) => l !== "");
const nonEmptyKey = (k: unknown): k is string => typeof k === "string" && k.trim() !== "";

export const readSearch = (v: unknown): SearchDraft => {
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
export const cleanSearch = (d: SearchDraft): Record<string, unknown> => {
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

/** 执行路径图节点。 */
function FlowNode({ title, sub, io }: { title: string; sub?: string; io?: boolean }) {
  return (
    <span
      className={cn(
        "rounded-md border px-2.5 py-1.5 text-sm font-semibold leading-tight whitespace-nowrap",
        io ? "border-input bg-secondary" : "border-input bg-card",
      )}
    >
      {title}
      {sub && <span className="block text-sm font-normal text-secondary">{sub}</span>}
    </span>
  );
}
function FlowArrow({ label, tone }: { label?: string; tone?: "ok" | "info" }) {
  return (
    <span className={cn("relative flex min-w-[30px] items-center justify-center gap-1 px-1", label && "min-w-[96px]")}>
      <span className="h-px min-w-[12px] flex-1 border-t border-dashed border-[var(--input)]" />
      <span className={cn("size-1.5 rotate-45 border-t border-r", tone === "ok" ? "border-[var(--success)]" : tone === "info" ? "border-primary" : "border-[var(--muted-foreground)]")} />
      {label && (
        <span className={cn("absolute -top-2 left-1/2 -translate-x-1/2 whitespace-nowrap text-sm", tone === "ok" ? "text-[var(--success)]" : tone === "info" ? "text-primary" : "text-destructive")}>
          {label}
        </span>
      )}
    </span>
  );
}


/** SearXNG 行控制器：状态与动作在容器（rail 切换不丢测速缓存），卡内沿用原名。 */
export type SearxController = {
  list: SearxInstance[];
  busy: boolean;
  error: string;
  add: string;
  setAdd: (v: string) => void;
  known: string[];
  enabledUrls: ReadonlySet<string>;
  test: Record<string, { testing: boolean; seconds: number | null; fail: boolean }>;
  allBusy: boolean;
  refresh: () => void;
  testOne: (url: string) => void;
  testAll: () => void;
  batch: (urls: string[]) => void;
  addUrl: () => void;
  delRow: (url: string) => void;
};

export function SearchCard({
  card,
  sourceList,
  keyedIds,
  searx,
  revealed,
  onToggleReveal,
  savedCard,
  saveError,
  onSave,
}: {
  card: ToolsCard<SearchDraft>;
  sourceList: SearchSource[];
  keyedIds: ReadonlySet<string>;
  searx: SearxController;
  revealed: ReadonlySet<string>;
  onToggleReveal: (keyId: string) => void;
  savedCard: "" | ToolsCardId;
  saveError: string;
  onSave: (c: ToolsCardId) => void;
}) {
  const { t } = useTranslation();
  const search = card.draft;
  const setSearch = card.setDraft;
  const dirty = { search: card.dirty, fetch: false };
  const toggleReveal = onToggleReveal;
  const searxList = searx.list;
  const searxBusy = searx.busy;
  const searxError = searx.error;
  const searxAdd = searx.add;
  const setSearxAdd = searx.setAdd;
  const searxKnown = searx.known;
  const searxTest = searx.test;
  const searxAllBusy = searx.allBusy;
  const enabledUrls = searx.enabledUrls;
  const testSearx = searx.testOne;
  const testSearxAll = searx.testAll;
  const refreshSearx = searx.refresh;
  const searxBatch = searx.batch;
  const searxAddUrl = searx.addUrl;
  const searxDelRow = searx.delRow;
  const sourceName = (id: string) => t(`tools.source.${id}`, id);
  const cardFooter = (c: "search" | "fetch", saveTestId: string) => (
    <CardFooter card={c} dirty={dirty[c]} savedCard={savedCard} saveError={saveError} saveCard={onSave} saveTestId={saveTestId} />
  );
  const cardHeader = (title: string, extra?: ReactNode) => <CardHeader title={title} extra={extra} />;
  const switchRow = (testid: string, checked: boolean, onToggle: () => void, label: string) => (
    <SwitchRow testid={testid} checked={checked} onToggle={onToggle} label={label} />
  );

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

  const setUrlsRaw = (raw: string) => setSearch((s) => ({ ...s, searxRaw: raw }));
  const toggleSearxUrl = (url: string) => {
    const lines = linesOf(search.searxRaw);
    setUrlsRaw((lines.includes(url) ? lines.filter((l) => l !== url) : [...lines, url]).join("\n"));
  };

  const flow = (() => {
    if (!search.enabled || search.sources.length === 0) {
      return (
        <div
          data-testid="tools-flow"
          data-state="off"
          className="flex items-center justify-center rounded-lg border border-dashed border-border px-4 py-5 text-sm text-secondary"
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
        <Button variant="outline" size="sm" data-testid="tools-searx-test-all" title={t("tools.searxTestAllHint")} disabled={searxAllBusy} onClick={() => void testSearxAll()}>
          {searxAllBusy ? t("tools.searxTesting") : t("tools.searxTestAll")}
        </Button>
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
                <span data-testid={`tools-searx-sec-${i}`} className="shrink-0 text-sm text-secondary">{t("tools.searxTesting")}</span>
              ) : (
                <span
                  data-testid={`tools-searx-sec-${i}`}
                  className={cn("shrink-0 font-mono text-sm", st?.fail ? "text-destructive" : "text-secondary")}
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
            if (e.key === "Enter" && !isImeComposing(e)) searxAddUrl();
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
                      <span className="grid size-5 place-items-center rounded bg-primary text-sm font-semibold text-primary-foreground">
                        {search.sources.indexOf(item.id) + 1}
                      </span>
                      <b className="text-sm font-semibold">{sourceName(item.id)}</b>
                      {keyedIds.has(item.id) && (
                        <span className="rounded bg-[var(--accent-subtle)] px-1 text-sm text-primary">key</span>
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
  return searchCard;
}
