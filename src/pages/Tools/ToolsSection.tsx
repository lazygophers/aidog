// ─── 「扩展 → 工具」页容器（票 57-03 拆分后）：rail 状态总览 + 未保存拦截 + 三卡装配 ──
// 卡片展示与草稿变换在 SearchCard / FetchCard / InstallCard；草稿 / 快照 / dirty /
// 保存样板在 useMcpToolsCard（hook 挂在容器层，rail 切换不卸载状态，行为与拆分前一致）。
// 设置键 scope=mcp_tools / key=settings。读取走 mcp_tools_settings_get（票 57-01 读时迁移）；
// keys 路径保持 `keys` 结尾（管理接口脱敏按该后缀匹配，secret_fields.rs）；api_key / keys
// 一律密码框（默认隐藏），加载回填的 `***` 提交原样保持原值。保存走 settingsApi
// （settings_set），SECRET_PATHS 脱敏 + RESTORERS 回填；每卡保存前重新 settingsApi.get
// 只覆盖本卡字段（读-改-写，防两卡互相覆盖）。
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { mcpApi } from "../../services/api/mcp";
import { mcpToolsApi, searchApi, type SearchSource, type SearxInstance } from "../../services/api/search";
import { registerNavGuard } from "../../utils/navGuard";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { cn } from "../../lib/utils";
import { FetchCard, type FetchDraft } from "./FetchCard";
import { InstallCard } from "./InstallCard";
import { SearchCard, cleanSearch, linesOf, readSearch, type SearchDraft, type SearxController } from "./SearchCard";
import { useMcpToolsCard, type ToolsCardId } from "./useToolsCard";

export type ToolId = "web_search" | "web_fetch";
export type ToolState = "configured" | "unconfigured" | "disabled";
type CardId = ToolId | "install";

const readFetchView = (f: unknown): FetchDraft => {
  const v = (f ?? {}) as { enabled?: unknown; format?: unknown };
  return {
    enabled: typeof v.enabled === "boolean" ? v.enabled : true,
    format: v.format === "html" || v.format === "text" ? v.format : "markdown",
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
  return "unconfigured";
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

export function ToolsSection() {
  const { t } = useTranslation();
  const [loadError, setLoadError] = useState("");
  const searchCard = useMcpToolsCard<SearchDraft>((v) => readSearch(v?.search), (d) => ({ search: cleanSearch(d) }));
  const fetchCard = useMcpToolsCard<FetchDraft>((v) => readFetchView(v?.fetch), (d) => ({ fetch: d }));
  const [sourceList, setSourceList] = useState<SearchSource[]>([]);
  const [selected, setSelected] = useState<CardId>("web_search");
  const [pendingNav, setPendingNav] = useState<{ proceed: () => void } | null>(null);
  const [savedCard, setSavedCard] = useState<ToolsCardId | "">("");
  // 最近一次持久化的卡值：rail 状态按已存配置算，草稿改动不影响状态显示
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
      for (const u of [...linesOf(searchCard.draft.searxRaw), ...searxList.map((x) => x.url)]) {
        if (!seen.has(u)) {
          seen.add(u);
          rows.push(u);
        }
      }
      return rows.length === prev.length ? prev : rows;
    });
  }, [searchCard.draft.searxRaw, searxList]);
  const enabledUrls = useMemo(() => new Set(linesOf(searchCard.draft.searxRaw)), [searchCard.draft.searxRaw]);
  const [installing, setInstalling] = useState(false);
  const [installMsg, setInstallMsg] = useState("");
  const [installOk, setInstallOk] = useState(false);

  const load = useCallback(async () => {
    try {
      // 票 57-01：读走 mcp_tools_settings_get——后端读时迁移旧形状，返回永远是新形状。
      const v = await mcpToolsApi.settings();
      searchCard.init(v ?? null);
      fetchCard.init(v ?? null);
      setSavedCard("");
      setSaveError("");
      setLoadError("");
    } catch (e) {
      setLoadError(String(e));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- init 稳定（read 为模块级函数）
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

  const dirty = {
    search: searchCard.dirty,
    fetch: fetchCard.dirty,
  };
  const unsavedCount = Number(dirty.search) + Number(dirty.fetch);

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

  // 每卡保存：hook 内部保存前重新 get，只覆盖本卡字段（读-改-写）。
  // 生图卡已移除（票 08）：generate_image 走平台分组的生图模型链；存量 `image` 键
  // 不在此清理（后端 serde 忽略未知字段，polish 归票 11）。
  const saveCard = async (card: ToolsCardId) => {
    setSavedCard("");
    setSaveError("");
    try {
      await (card === "search" ? searchCard.save() : fetchCard.save());
      setSavedCard(card);
    } catch (e) {
      setSaveError(String(e));
    }
  };

  // ── SearXNG 行控制器（状态在容器，动作传给搜索卡）──
  const toggleReveal = (keyId: string) =>
    setRevealed((prev) => {
      const next = new Set(prev);
      if (next.has(keyId)) next.delete(keyId);
      else next.add(keyId);
      return next;
    });

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
  const [searxAllBusy, setSearxAllBusy] = useState(false);
  const testSearxAll = async () => {
    setSearxAllBusy(true);
    try {
      let next = 0;
      await Promise.all(Array.from({ length: Math.min(5, searxKnown.length) }, async () => {
        while (next < searxKnown.length) {
          const url = searxKnown[next++];
          await testSearx(url);
        }
      }));
    } finally {
      setSearxAllBusy(false);
    }
  };
  const searxAddUrl = () => {
    const u = searxAdd.trim();
    if (u === "") return;
    searchCard.setDraft((s) => ({ ...s, searxRaw: [...new Set([...linesOf(s.searxRaw), u])].join("\n") }));
    setSearxAdd("");
  };
  const searxDelRow = (url: string) => {
    searchCard.setDraft((s) => ({ ...s, searxRaw: linesOf(s.searxRaw).filter((l) => l !== url).join("\n") }));
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
        t("tools.installed", "Written to {{name}} ({{agents}}) config — restart the agent to take effect", {
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

  // ── rail 状态（按已存值）──
  const liveSettings = useMemo(
    () => ({ search: searchCard.saved, fetch: fetchCard.saved }),
    [searchCard.saved, fetchCard.saved],
  );
  const statusOf = (tool: ToolId) => toolStatus(tool, liveSettings, keyedIds);

  const searx: SearxController = {
    list: searxList,
    busy: searxBusy,
    error: searxError,
    add: searxAdd,
    setAdd: setSearxAdd,
    known: searxKnown,
    enabledUrls,
    test: searxTest,
    allBusy: searxAllBusy,
    refresh: () => void refreshSearx(),
    testOne: (url) => void testSearx(url),
    testAll: () => void testSearxAll(),
    batch: (urls) => searchCard.setDraft((s) => ({ ...s, searxRaw: urls.join("\n") })),
    addUrl: searxAddUrl,
    delRow: searxDelRow,
  };

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
    { id: "install", title: t("tools.card.install"), summary: t("tools.installHint").slice(0, 24) + "…" },
  ];

  const detailCard =
    selected === "web_search" ? (
      <SearchCard
        card={searchCard}
        sourceList={sourceList}
        keyedIds={keyedIds}
        searx={searx}
        revealed={revealed}
        onToggleReveal={toggleReveal}
        savedCard={savedCard}
        saveError={saveError}
        onSave={saveCard}
      />
    ) : selected === "web_fetch" ? (
      <FetchCard card={fetchCard} savedCard={savedCard} saveError={saveError} onSave={saveCard} />
    ) : (
      <InstallCard installing={installing} installMsg={installMsg} installOk={installOk} onInstall={() => void install()} />
    );

  return (
    <div data-testid="mcp-tools-section" className="flex w-full flex-col gap-4">
      <div>
        <div className="section-title">{t("tools.title", "Tools")}</div>
        <div className="mt-1 text-sm text-secondary">{t("tools.desc")}</div>
      </div>

      {loadError && (
        <div className="text-sm text-destructive" data-testid="mcp-tools-load-error">{loadError}</div>
      )}

      {unsavedCount > 0 && (
        <div
          data-testid="tools-unsaved-banner"
          className="flex items-center gap-1.5 rounded-lg border border-[color-mix(in_srgb,var(--warning)_30%,transparent)] bg-[var(--color-warning-bg)] px-3 py-2 text-sm text-[var(--warning)]"
        >
          <span className="size-1.5 rounded-full bg-current" />
          {t("tools.unsaved.banner", { count: unsavedCount })}
        </div>
      )}

      <div className="grid grid-cols-1 items-start gap-5 lg:grid-cols-[268px_1fr]">
        <aside className="flex flex-col gap-1.5">
          <div className="mb-0.5 px-0.5 font-mono text-sm text-secondary">{t("tools.nav.title")}</div>
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
                  {item.summary && <small className="block text-sm text-secondary">{item.summary}</small>}
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
