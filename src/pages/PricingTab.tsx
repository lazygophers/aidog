import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  modelPriceApi,
  priceSyncApi,
  type ModelPriceSummary,
  type PriceSyncSettings,
  type PriceSyncResult,
  type ModelPriceFilter,
} from "../services/api";
import { IconClose } from "../components/icons";

// ponytail: 统一 token (title 15→20, body 14→15), 视觉差异可忽略
import { F } from "../domains/shared/tokens";
import { formatDateTime } from "../utils/formatters";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// radix Select 的 SelectItem value="" 会抛错 → 用 __none__ 哨兵映射回空值（= 不筛选）
const NONE = "__none__";

const PAGE_SIZE_OPTIONS = [20, 50, 100, 200];

export function PricingTab() {
  const { t } = useTranslation();
  const [prices, setPrices] = useState<ModelPriceSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const [jumpPage, setJumpPage] = useState("");
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [syncing, setSyncing] = useState(false);
  const [syncSettings, setSyncSettings] = useState<PriceSyncSettings>({
    auto_sync_enabled: false,
    sync_interval_secs: 86400,
    last_sync_at: 0,
    fallback_input_price: 3.0,
    fallback_output_price: 3.0,
  });

  // ── Filter state ──
  const [filterQuery, setFilterQuery] = useState("");
  const [filterSource, setFilterSource] = useState("");

  const activeFilter = useMemo<ModelPriceFilter>(() => {
    const f: ModelPriceFilter = {};
    if (filterQuery.trim()) f.query = filterQuery.trim();
    if (filterSource) f.source = filterSource;
    return f;
  }, [filterQuery, filterSource]);

  const hasFilter = !!(filterQuery.trim() || filterSource);

  const loadSettings = useCallback(async () => {
    try {
      const s = await priceSyncApi.get();
      setSyncSettings(s);
    } catch { /* defaults */ }
  }, []);

  const load = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    try {
      const [items, count] = await Promise.all([
        modelPriceApi.listFiltered(activeFilter, pageSize, offset),
        modelPriceApi.countFiltered(activeFilter),
      ]);
      setPrices(items || []);
      setTotal(count);
    } catch (e) { console.error(e); }
    if (!silent) setLoading(false);
  }, [offset, pageSize, activeFilter]);

  useEffect(() => { loadSettings(); }, [loadSettings]);
  useEffect(() => { load(); }, [load]);

  // Reset offset when filter or page size changes
  useEffect(() => { setOffset(0); }, [hasFilter, activeFilter, pageSize]);

  const handleSync = async () => {
    setSyncing(true);
    setMessage("");
    try {
      const result: PriceSyncResult = await modelPriceApi.sync();
      setMessage(
        t("pricing.syncResult", "同步完成: +{added} 新增, ~{updated} 更新, {failed} 失败 (共 {total} 模型)")
          .replace("{added}", String(result.added))
          .replace("{updated}", String(result.updated))
          .replace("{failed}", String(result.failed))
          .replace("{total}", String(result.total))
      );
      await loadSettings();
      load();
    } catch (e: any) {
      setMessage(e.toString());
    } finally {
      setSyncing(false);
    }
  };

  const updateSettings = async (partial: Partial<PriceSyncSettings>) => {
    const next = { ...syncSettings, ...partial };
    setSyncSettings(next);
    try {
      await priceSyncApi.set(next);
    } catch (e: any) { setMessage(e.toString()); }
  };

  const clearFilter = () => {
    setFilterQuery("");
    setFilterSource("");
  };

  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const currentPage = Math.min(Math.floor(offset / pageSize) + 1, totalPages);

  const handleJumpPage = () => {
    const p = parseInt(jumpPage, 10);
    if (p >= 1 && p <= totalPages) {
      setOffset((p - 1) * pageSize);
      setJumpPage("");
    }
  };

  const formatPrice = (v: number | null) => {
    if (v == null) return "-";
    if (v < 0.01) return v.toFixed(4);
    if (v < 1) return v.toFixed(3);
    return v.toFixed(2);
  };

  /** token 数 → 千分位 K 展示；null → "-" */
  const formatTokens = (v: number | null | undefined) => {
    if (v == null) return "-";
    if (v >= 1000) return `${(v / 1000).toLocaleString()}K`;
    return String(v);
  };

  const formatTime = (ts: number) => {
    if (!ts) return "-";
    return formatDateTime(ts) || "-";
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* Sync controls */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div style={{ fontSize: F.body, fontWeight: 600 }}>{t("pricing.syncTitle", "价格同步")}</div>
            <div className="text-secondary" style={{ fontSize: F.small, marginTop: 2 }}>
              {t("pricing.syncDesc", "从 GitHub 同步模型价格 + max_tokens（含各平台价格）")}
            </div>
          </div>
          <Button onClick={handleSync} disabled={syncing} style={{ fontSize: F.hint, height: "auto", padding: "4px 10px" }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              {syncing ? t("pricing.syncing", "同步中...") : t("pricing.syncNow", "立即同步")}
              {syncing && (
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="spin">
                  <path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" />
                </svg>
              )}
            </span>
          </Button>
        </div>

        {/* Sync settings row */}
        <div style={{ display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap", paddingTop: 8, borderTop: "1px solid var(--border)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <div
              className={`toggle ${syncSettings.auto_sync_enabled ? "active" : ""}`}
              onClick={() => updateSettings({ auto_sync_enabled: !syncSettings.auto_sync_enabled })}
              role="switch"
              aria-checked={syncSettings.auto_sync_enabled}
              tabIndex={0}
            />
            <span style={{ fontSize: F.small, fontWeight: 600 }}>{t("pricing.autoSync", "自动同步")}</span>
          </div>
          {syncSettings.auto_sync_enabled && (
            <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
              <label style={{ fontSize: F.small, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                {t("pricing.interval", "间隔")}
              </label>
              <Select
                value={String(syncSettings.sync_interval_secs)}
                onValueChange={(v) => updateSettings({ sync_interval_secs: Number(v) })}
              >
                <SelectTrigger style={{ padding: "3px 6px", fontSize: F.small, width: 100, height: 30 }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="3600">1h</SelectItem>
                  <SelectItem value="21600">6h</SelectItem>
                  <SelectItem value="43200">12h</SelectItem>
                  <SelectItem value="86400">24h</SelectItem>
                  <SelectItem value="604800">7d</SelectItem>
                </SelectContent>
              </Select>
            </div>
          )}
          <span style={{ fontSize: F.small, color: "var(--text-tertiary)", marginLeft: "auto" }}>
            {t("pricing.lastSync", "上次同步")}: {formatTime(syncSettings.last_sync_at)}
          </span>
        </div>

        {/* Fallback price */}
        <div style={{ display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap", paddingTop: 8, borderTop: "1px solid var(--border)" }}>
          <span style={{ fontSize: F.small, fontWeight: 600 }}>{t("pricing.fallback", "兜底价格 $/M tokens")}</span>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <label style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{t("pricing.input", "输入")}</label>
            <Input
              type="number" min={0} step={0.1}
              value={syncSettings.fallback_input_price}
              onChange={(e) => updateSettings({ fallback_input_price: Math.max(0, Number(e.target.value)) })}
              style={{ width: 70, padding: "3px 6px", fontSize: F.small, height: 30 }}
            />
          </div>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <label style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{t("pricing.output", "输出")}</label>
            <Input
              type="number" min={0} step={0.1}
              value={syncSettings.fallback_output_price}
              onChange={(e) => updateSettings({ fallback_output_price: Math.max(0, Number(e.target.value)) })}
              style={{ width: 70, padding: "3px 6px", fontSize: F.small, height: 30 }}
            />
          </div>
        </div>
      </div>

      {/* Filter bar */}
      <div className="glass-surface" style={{ padding: "10px 16px", display: "flex", flexWrap: "wrap", gap: 10, alignItems: "center" }}>
        <Input
          placeholder={t("pricing.searchPlaceholder", "搜索模型名称...")}
          value={filterQuery}
          onChange={(e) => setFilterQuery(e.target.value)}
          style={{ flex: "1 1 160px", fontSize: F.small, padding: "6px 10px", height: 32 }}
        />
        <Select
          value={filterSource || NONE}
          onValueChange={(v) => setFilterSource(v === NONE ? "" : v)}
        >
          <SelectTrigger style={{ fontSize: F.small, padding: "6px 8px", width: 110, height: 32 }}>
            <SelectValue placeholder={t("pricing.allSources", "全部来源")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={NONE}>{t("pricing.allSources", "全部来源")}</SelectItem>
            <SelectItem value="github">GitHub</SelectItem>
            <SelectItem value="manual">{t("pricing.manual", "手动")}</SelectItem>
          </SelectContent>
        </Select>
        {hasFilter && (
          <Button variant="ghost" onClick={clearFilter} style={{ fontSize: F.small, padding: "4px 8px", height: "auto", color: "var(--text-tertiary)" }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}><IconClose size={11} /> {t("pricing.clearFilter", "清除")}</span>
          </Button>
        )}
      </div>

      {/* Price table */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : prices.length === 0 ? (
        <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
          <div className="text-tertiary" style={{ fontSize: F.hint }}>
            {t("pricing.empty", "暂无价格数据，请点击「立即同步」从 GitHub 获取")}
          </div>
        </div>
      ) : (
        <>
          <div className="glass-surface" style={{ overflow: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: F.hint }}>
              <thead>
                <tr style={{ borderBottom: "1px solid var(--border)" }}>
                  <ThCell>{t("pricing.model", "模型")}</ThCell>
                  <ThCell>{t("pricing.source", "来源")}</ThCell>
                  <ThCell>{t("pricing.defaultPlatform", "默认平台")}</ThCell>
                  <ThCell>{t("pricing.inputPrice", "输入 $/M")}</ThCell>
                  <ThCell>{t("pricing.outputPrice", "输出 $/M")}</ThCell>
                  <ThCell>{t("pricing.cacheReadPrice", "缓存读取 $/M")}</ThCell>
                  <ThCell>{t("pricing.contextWindow", "上下文")}</ThCell>
                  <ThCell>{t("pricing.maxOutput", "最大输出")}</ThCell>
                </tr>
              </thead>
              <tbody>
                {prices.map((p) => (
                  <tr key={p.id} style={{ borderBottom: "1px solid var(--border)" }}>
                    <TdCell><span style={{ fontWeight: 500, fontSize: F.small }}>{p.model_name}</span></TdCell>
                    <TdCell>
                      <Badge variant="secondary" style={{
                        fontSize: 10,
                        padding: "1px 6px",
                        background: p.source === "manual" ? "var(--accent-subtle)" : "var(--bg-secondary)",
                        color: p.source === "manual" ? "var(--accent)" : "var(--text-secondary)",
                        border: "none",
                      }}>
                        {p.source}
                      </Badge>
                    </TdCell>
                    <TdCell><span style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{p.default_platform || "-"}</span></TdCell>
                    <TdCell>{formatPrice(p.input_price)}</TdCell>
                    <TdCell>{formatPrice(p.output_price)}</TdCell>
                    <TdCell>{formatPrice(p.cache_read_price)}</TdCell>
                    <TdCell><span style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{formatTokens(p.context_window)}</span></TdCell>
                    <TdCell><span style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{formatTokens(p.max_output_tokens)}</span></TdCell>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          <Pagination
            currentPage={currentPage}
            totalPages={totalPages}
            total={total}
            pageSize={pageSize}
            pageSizeOptions={PAGE_SIZE_OPTIONS}
            jumpPage={jumpPage}
            onJumpPageChange={setJumpPage}
            onJump={handleJumpPage}
            onPageSizeChange={ps => { setPageSize(ps); setOffset(0); }}
            onPageChange={page => setOffset((page - 1) * pageSize)}
          />
        </>
      )}

      {message && <div className="toast">{message}</div>}
    </div>
  );
}

/** 分页导航：页码按钮 + 跳页 + 每页数量 */
function Pagination({
  currentPage, totalPages, total, pageSize, pageSizeOptions,
  jumpPage, onJumpPageChange, onJump, onPageSizeChange, onPageChange,
}: {
  currentPage: number;
  totalPages: number;
  total: number;
  pageSize: number;
  pageSizeOptions: number[];
  jumpPage: string;
  onJumpPageChange: (v: string) => void;
  onJump: () => void;
  onPageSizeChange: (size: number) => void;
  onPageChange: (page: number) => void;
}) {
  const rangeStart = (currentPage - 1) * pageSize + 1;
  const rangeEnd = Math.min(currentPage * pageSize, total);

  const pages: (number | "ellipsis")[] = [];
  if (totalPages <= 7) {
    for (let i = 1; i <= totalPages; i++) pages.push(i);
  } else {
    pages.push(1);
    if (currentPage > 3) pages.push("ellipsis");
    const start = Math.max(2, currentPage - 1);
    const end = Math.min(totalPages - 1, currentPage + 1);
    for (let i = start; i <= end; i++) pages.push(i);
    if (currentPage < totalPages - 2) pages.push("ellipsis");
    pages.push(totalPages);
  }

  const btnStyle: React.CSSProperties = {
    fontSize: 12, padding: "4px 8px", minWidth: 28, textAlign: "center",
  };

  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
      {/* Left: range info + page size */}
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span className="text-tertiary" style={{ fontSize: 12 }}>
          {rangeStart}–{rangeEnd} / {total}
        </span>
        <Select
          value={String(pageSize)}
          onValueChange={v => onPageSizeChange(Number(v))}
        >
          <SelectTrigger style={{ fontSize: 12, padding: "2px 6px", width: 80, height: 28 }}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {pageSizeOptions.map(s => <SelectItem key={s} value={String(s)}>{s}/page</SelectItem>)}
          </SelectContent>
        </Select>
      </div>

      {/* Right: page nav + jump */}
      <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
        <Button variant="ghost" style={btnStyle} disabled={currentPage <= 1}
          onClick={() => onPageChange(1)} title="First">⟪</Button>
        <Button variant="ghost" style={btnStyle} disabled={currentPage <= 1}
          onClick={() => onPageChange(currentPage - 1)}>←</Button>
        {pages.map((p, i) =>
          p === "ellipsis" ? (
            <span key={`e${i}`} className="text-tertiary" style={{ fontSize: 12, padding: "0 4px" }}>…</span>
          ) : (
            <Button key={p} variant={p === currentPage ? "default" : "ghost"}
              style={{
                ...btnStyle,
                ...(p === currentPage ? { fontWeight: 700, color: "var(--accent)" } : {}),
              }}
              onClick={() => onPageChange(p)}>{p}</Button>
          ),
        )}
        <Button variant="ghost" style={btnStyle} disabled={currentPage >= totalPages}
          onClick={() => onPageChange(currentPage + 1)}>→</Button>
        <Button variant="ghost" style={btnStyle} disabled={currentPage >= totalPages}
          onClick={() => onPageChange(totalPages)} title="Last">⟫</Button>

        {/* Jump to page */}
        <div style={{ display: "flex", alignItems: "center", gap: 4, marginLeft: 8 }}>
          <Input
            type="number"
            min={1}
            max={totalPages}
            value={jumpPage}
            onChange={e => onJumpPageChange(e.target.value)}
            onKeyDown={e => { if (e.key === "Enter") onJump(); }}
            placeholder="#"
            style={{ width: 50, fontSize: 12, padding: "3px 6px", textAlign: "center", height: 28 }}
          />
          <Button variant="ghost" style={btnStyle} onClick={onJump}>
            Go
          </Button>
        </div>
      </div>
    </div>
  );
}

function ThCell({ children }: { children: React.ReactNode }) {
  return (
    <th style={{
      padding: "8px 12px", textAlign: "left", fontWeight: 600,
      color: "var(--text-secondary)", whiteSpace: "nowrap", fontSize: 12,
    }}>
      {children}
    </th>
  );
}

function TdCell({ children }: { children: React.ReactNode }) {
  return (
    <td style={{ padding: "8px 12px", whiteSpace: "nowrap" }}>
      {children}
    </td>
  );
}
