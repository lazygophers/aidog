// 「模型信息」页（原 PricingTab 位置）：同步状态区 + 模型维度 / 平台维度双 tab 列表 + 详情弹窗。
//
// 数据源：`model_info_snapshot` 一次 RPC 拿全（聚合行 + 平台预设），前端不做二次拼装、不分页请求；
// 平台展示名 / logo 走 registry 读取层（getProtocolLabelMap / ProtocolLogo），UI 不再写回落分支。

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  modelInfoApi,
  modelPriceApi,
  priceSyncApi,
  type ModelEntry,
  type ModelEntryGroup,
  type ModelInfoSnapshot,
  type PriceSyncResult,
  type PriceSyncSettings,
  type Protocol,
} from "../../services/api";
import { getProtocolLabelMap } from "../../domains/platforms/defaults";
import { ProtocolLogo } from "../../domains/platforms/ProtocolLogo";
import { F } from "../../domains/shared/tokens";
import { useReveal } from "../../components/shared";
import { IconClose } from "../../components/icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { SyncStatusCard } from "./SyncStatusCard";
import { ModelDetailDialog } from "./ModelDetailDialog";
import { CAPABILITIES, CapabilityBadges, capabilityLabel } from "./CapabilityBadges";
import { PAGE_SIZE_OPTIONS, Pagination } from "./Pagination";
import { ModelNameCell } from "./ModelName";
import { fmtPricePerM, fmtTokens, parsePriceData } from "./priceData";

// radix Select 的 SelectItem value="" 会抛错 → __none__ 哨兵映射回「不筛选」
const NONE = "__none__";

const DEFAULT_SYNC_SETTINGS: PriceSyncSettings = {
  auto_sync_enabled: false,
  sync_interval_secs: 86400,
  last_sync_at: 0,
  registry_last_updated: 0,
  fallback_input_price: 3.0,
  fallback_output_price: 3.0,
};

export function ModelInfoTab() {
  const { t, i18n } = useTranslation();
  const [snapshot, setSnapshot] = useState<ModelInfoSnapshot | null>(null);
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [syncResult, setSyncResult] = useState<PriceSyncResult | null>(null);
  const [message, setMessage] = useState("");
  const [settings, setSettings] = useState<PriceSyncSettings>(DEFAULT_SYNC_SETTINGS);

  // ── 筛选 / 分页 / 选中 ──
  const [query, setQuery] = useState("");
  const [platformFilter, setPlatformFilter] = useState("");
  const [capabilityFilter, setCapabilityFilter] = useState("");
  const [officialOnly, setOfficialOnly] = useState(false);
  const [pageSize, setPageSize] = useState(50);
  const [page, setPage] = useState(1);
  const [jumpPage, setJumpPage] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [activePlatform, setActivePlatform] = useState<string | null>(null);

  const filterCard = useReveal<HTMLDivElement>(80);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const snap = await modelInfoApi.snapshot();
      setSnapshot(snap);
    } catch (e) {
      console.error(e);
      setMessage(String(e));
    }
    setLoading(false);
  }, []);

  useEffect(() => { load(); }, [load]);

  useEffect(() => {
    priceSyncApi.get().then(setSettings).catch(() => { /* 用默认值 */ });
  }, []);

  useEffect(() => {
    let cancelled = false;
    getProtocolLabelMap(i18n.language).then(m => { if (!cancelled) setLabelMap(m); });
    return () => { cancelled = true; };
  }, [i18n.language]);

  const groups = useMemo(() => snapshot?.groups ?? [], [snapshot]);

  /** platform_code → 该平台全部模型条目（平台维度 tab 的数据源，由聚合行反向展开）。 */
  const byPlatform = useMemo(() => {
    const m = new Map<string, ModelEntry[]>();
    for (const g of groups) {
      for (const e of g.entries) {
        const list = m.get(e.platform_code);
        if (list) list.push(e);
        else m.set(e.platform_code, [e]);
      }
    }
    for (const list of m.values()) list.sort((a, b) => a.model_id.localeCompare(b.model_id));
    return m;
  }, [groups]);

  /** `index.json` 的 pricing_only 来源（litellm / meta / mistral）：只提供比价条目，
   *  没有 platform.json，用户在平台页根本选不到，不能出现在平台筛选与平台维度列表里。 */
  const pricingOnly = useMemo(
    () => new Set(snapshot?.pricing_only ?? []),
    [snapshot],
  );

  const platformCodes = useMemo(
    () => [...byPlatform.keys()]
      .filter(code => !pricingOnly.has(code))
      .sort((a, b) => (labelMap[a] ?? a).localeCompare(labelMap[b] ?? b)),
    [byPlatform, labelMap, pricingOnly],
  );

  const hasFilter = !!(query.trim() || platformFilter || capabilityFilter || officialOnly);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return groups.filter(g => {
      if (platformFilter && !g.entries.some(e => e.platform_code === platformFilter)) return false;
      if (capabilityFilter && !g.entries.some(e => e.capabilities.includes(capabilityFilter))) return false;
      if (officialOnly && !g.entries.some(e => e.official)) return false;
      if (!q) return true;
      if (g.display_name.toLowerCase().includes(q)) return true;
      if (g.canonical_model.toLowerCase().includes(q)) return true;
      return g.entries.some(e => e.model_id.toLowerCase().includes(q));
    });
  }, [groups, query, platformFilter, capabilityFilter, officialOnly]);

  useEffect(() => { setPage(1); }, [query, platformFilter, capabilityFilter, officialOnly, pageSize]);

  const totalPages = Math.max(1, Math.ceil(filtered.length / pageSize));
  const currentPage = Math.min(page, totalPages);
  const pageRows = useMemo(
    () => filtered.slice((currentPage - 1) * pageSize, currentPage * pageSize),
    [filtered, currentPage, pageSize],
  );

  const selectedGroup = useMemo(
    () => groups.find(g => g.canonical_model === selected) ?? null,
    [groups, selected],
  );

  const handleSync = async () => {
    setSyncing(true);
    setMessage("");
    try {
      const result = await modelPriceApi.sync();
      setSyncResult(result);
      const s = await priceSyncApi.get().catch(() => settings);
      setSettings(s);
      await load();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setSyncing(false);
    }
  };

  const updateSettings = async (partial: Partial<PriceSyncSettings>) => {
    const next = { ...settings, ...partial };
    setSettings(next);
    try {
      await priceSyncApi.set(next);
    } catch (e) { setMessage(String(e)); }
  };

  const clearFilter = () => {
    setQuery("");
    setPlatformFilter("");
    setCapabilityFilter("");
    setOfficialOnly(false);
  };

  const handleJumpPage = () => {
    const p = parseInt(jumpPage, 10);
    if (p >= 1 && p <= totalPages) { setPage(p); setJumpPage(""); }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      <SyncStatusCard
        settings={settings}
        onUpdateSettings={updateSettings}
        syncing={syncing}
        onSync={handleSync}
        result={syncResult}
        bundled={!!snapshot?.bundled}
      />

      {/* 筛选栏（两个 tab 共用；平台维度下平台下拉退化为左侧平台列表的选中态） */}
      <div ref={filterCard.ref} className={`glass-surface hover-lift reveal${filterCard.shown ? " in" : ""}`} style={{ padding: "10px 16px", display: "flex", flexWrap: "wrap", gap: 10, alignItems: "center" }}>
        <Input
          placeholder={t("modelInfo.searchPlaceholder")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          style={{ flex: "1 1 180px", fontSize: F.small, padding: "6px 10px", height: 32 }}
        />
        <Select value={platformFilter || NONE} onValueChange={(v) => setPlatformFilter(v === NONE ? "" : v)}>
          <SelectTrigger style={{ fontSize: F.small, padding: "6px 8px", width: 160, height: 32 }}>
            <SelectValue placeholder={t("modelInfo.allPlatforms")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={NONE}>{t("modelInfo.allPlatforms")}</SelectItem>
            {platformCodes.map(code => (
              <SelectItem key={code} value={code}>{labelMap[code] ?? code}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={capabilityFilter || NONE} onValueChange={(v) => setCapabilityFilter(v === NONE ? "" : v)}>
          <SelectTrigger style={{ fontSize: F.small, padding: "6px 8px", width: 140, height: 32 }}>
            <SelectValue placeholder={t("modelInfo.allCapabilities")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={NONE}>{t("modelInfo.allCapabilities")}</SelectItem>
            {CAPABILITIES.map(c => (
              <SelectItem key={c} value={c}>{capabilityLabel(t, c)}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        <label style={{ display: "inline-flex", alignItems: "center", gap: 6, fontSize: F.small }}>
          <Switch checked={officialOnly} onCheckedChange={setOfficialOnly} />
          {t("modelInfo.officialOnly")}
        </label>
        {hasFilter && (
          <Button variant="ghost" onClick={clearFilter} style={{ fontSize: F.small, padding: "4px 8px", height: "auto", color: "var(--text-tertiary)" }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}><IconClose size={11} /> {t("modelInfo.clearFilter")}</span>
          </Button>
        )}
      </div>

      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : groups.length === 0 ? (
        <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
          <div className="text-tertiary" style={{ fontSize: F.hint }}>{t("modelInfo.empty")}</div>
        </div>
      ) : (
        <Tabs defaultValue="models">
          <TabsList>
            <TabsTrigger value="models">{t("modelInfo.tabModels")}</TabsTrigger>
            <TabsTrigger value="platforms">{t("modelInfo.tabPlatforms")}</TabsTrigger>
          </TabsList>

          <TabsContent value="models">
            <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
              <div className="glass-surface" style={{ overflow: "auto" }}>
                <Table className="glass-table" style={{ fontSize: F.hint }}>
                  <TableHeader>
                    <TableRow>
                      <Th>{t("modelInfo.colModel")}</Th>
                      <Th>{t("modelInfo.colPlatform")}</Th>
                      <Th>{t("modelInfo.colCapabilities")}</Th>
                      <Th>{t("modelInfo.colContext")}</Th>
                      <Th>{t("modelInfo.colInput")}</Th>
                      <Th>{t("modelInfo.colOutput")}</Th>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {pageRows.map(g => (
                      <ModelRow
                        key={g.canonical_model}
                        group={g}
                        labelMap={labelMap}
                        onOpen={() => setSelected(g.canonical_model)}
                      />
                    ))}
                  </TableBody>
                </Table>
              </div>
              <Pagination
                currentPage={currentPage}
                totalPages={totalPages}
                total={filtered.length}
                pageSize={pageSize}
                pageSizeOptions={PAGE_SIZE_OPTIONS}
                jumpPage={jumpPage}
                onJumpPageChange={setJumpPage}
                onJump={handleJumpPage}
                onPageSizeChange={ps => { setPageSize(ps); setPage(1); }}
                onPageChange={setPage}
              />
            </div>
          </TabsContent>

          <TabsContent value="platforms">
            <PlatformPane
              platformCodes={platformCodes}
              byPlatform={byPlatform}
              labelMap={labelMap}
              active={activePlatform}
              onSelect={setActivePlatform}
              query={query}
            />
          </TabsContent>
        </Tabs>
      )}

      <ModelDetailDialog group={selectedGroup} labelMap={labelMap} pricingOnly={pricingOnly} onClose={() => setSelected(null)} />

      {message && <div className="toast">{message}</div>}
    </div>
  );
}

/** 模型维度一行：展示名 + 真实请求名 + 代表条目（primary_platform）的能力/上下文/价格。 */
function ModelRow({ group, labelMap, onOpen }: {
  group: ModelEntryGroup;
  labelMap: Record<string, string>;
  onOpen: () => void;
}) {
  const { t } = useTranslation();
  const primary = group.entries.find(e => e.platform_code === group.primary_platform) ?? group.entries[0];
  const price = parsePriceData(primary?.price_data ?? "");
  const extra = group.entries.length - 1;

  return (
    <TableRow
      className="hover-lift"
      style={{ cursor: "pointer" }}
      onClick={onOpen}
      title={group.canonical_model}
    >
      <Td>
        <ModelNameCell displayName={group.display_name} modelId={primary?.model_id ?? ""} />
      </Td>
      <Td>
        <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
          {primary && <ProtocolLogo protocol={primary.platform_code as Protocol} size={16} />}
          <span style={{ fontSize: F.small }}>
            {primary ? (labelMap[primary.platform_code] ?? primary.platform_code) : "-"}
          </span>
          {primary?.official && (
            <Badge variant="secondary" style={{ fontSize: 10, padding: "1px 5px", border: "none" }}>
              {t("modelInfo.official")}
            </Badge>
          )}
          {extra > 0 && (
            <span className="text-tertiary" style={{ fontSize: 11 }}>
              {t("modelInfo.morePlatforms").replace("{count}", String(extra))}
            </span>
          )}
        </span>
      </Td>
      <Td><CapabilityBadges capabilities={primary?.capabilities ?? []} /></Td>
      <Td><span className="text-secondary" style={{ fontSize: F.small }}>{fmtTokens(primary?.context_window)}</span></Td>
      <Td>{fmtPricePerM(price.input_cost_per_token)}</Td>
      <Td>{fmtPricePerM(price.output_cost_per_token)}</Td>
    </TableRow>
  );
}

/** 平台维度：左侧平台清单（logo + 名称 + 模型数），右侧该平台全部模型条目。 */
function PlatformPane({ platformCodes, byPlatform, labelMap, active, onSelect, query }: {
  platformCodes: string[];
  byPlatform: Map<string, ModelEntry[]>;
  labelMap: Record<string, string>;
  active: string | null;
  onSelect: (code: string) => void;
  query: string;
}) {
  const { t } = useTranslation();
  const q = query.trim().toLowerCase();
  const visibleCodes = q
    ? platformCodes.filter(c => c.includes(q) || (labelMap[c] ?? "").toLowerCase().includes(q))
    : platformCodes;
  const current = active && byPlatform.has(active) ? active : null;
  const entries = current ? (byPlatform.get(current) ?? []) : [];

  return (
    <div style={{ display: "flex", gap: 12, alignItems: "flex-start", flexWrap: "wrap" }}>
      <div className="glass-surface" style={{ flex: "0 0 220px", maxHeight: 520, overflow: "auto", padding: 6 }}>
        {visibleCodes.map(code => (
          <button
            key={code}
            onClick={() => onSelect(code)}
            style={{
              width: "100%", display: "flex", alignItems: "center", gap: 8, padding: "6px 8px",
              background: code === current ? "var(--accent-subtle)" : "transparent",
              border: "none", borderRadius: "var(--radius-sm)", cursor: "pointer",
              // 选中态只靠 13% 透明的 accent-subtle 底色，浅色模式下几乎看不出选了哪一项；
              // 补一条 accent 竖线 + 文字提权，两种模式下都能一眼定位。
              boxShadow: code === current ? "inset 2px 0 0 var(--accent)" : "none",
              color: code === current ? "var(--accent)" : "inherit",
              fontWeight: code === current ? 600 : 400,
              textAlign: "start",
            }}
          >
            <ProtocolLogo protocol={code as Protocol} size={16} />
            <span style={{ fontSize: F.small, flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {labelMap[code] ?? code}
            </span>
            <span className="text-tertiary" style={{ fontSize: 11 }}>{byPlatform.get(code)?.length ?? 0}</span>
          </button>
        ))}
      </div>

      <div className="glass-surface" style={{ flex: "1 1 420px", overflow: "auto" }}>
        {current === null ? (
          <div className="text-tertiary" style={{ padding: 30, textAlign: "center", fontSize: F.hint }}>
            {t("modelInfo.selectPlatform")}
          </div>
        ) : (
          <Table className="glass-table" style={{ fontSize: F.hint }}>
            <TableHeader>
              <TableRow>
                <Th>{t("modelInfo.colModel")}</Th>
                <Th>{t("modelInfo.colCapabilities")}</Th>
                <Th>{t("modelInfo.colContext")}</Th>
                <Th>{t("modelInfo.colInput")}</Th>
                <Th>{t("modelInfo.colOutput")}</Th>
              </TableRow>
            </TableHeader>
            <TableBody>
              {entries.map(e => {
                const price = parsePriceData(e.price_data);
                return (
                  <TableRow key={e.model_id} className="hover-lift">
                    <Td>
                      <ModelNameCell displayName={e.display_name} modelId={e.model_id} />
                    </Td>
                    <Td><CapabilityBadges capabilities={e.capabilities} /></Td>
                    <Td><span className="text-secondary" style={{ fontSize: F.small }}>{fmtTokens(e.context_window)}</span></Td>
                    <Td>{fmtPricePerM(price.input_cost_per_token)}</Td>
                    <Td>{fmtPricePerM(price.output_cost_per_token)}</Td>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        )}
      </div>
    </div>
  );
}

function Th({ children }: { children: React.ReactNode }) {
  return (
    <TableHead style={{
      padding: "8px 12px", fontWeight: 600,
      color: "var(--text-secondary)", whiteSpace: "nowrap", fontSize: 12, height: "auto",
    }}>
      {children}
    </TableHead>
  );
}

function Td({ children }: { children: React.ReactNode }) {
  return <TableCell style={{ padding: "8px 12px", whiteSpace: "nowrap" }}>{children}</TableCell>;
}
