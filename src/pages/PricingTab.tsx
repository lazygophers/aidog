import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  modelPriceApi,
  priceSyncApi,
  type ModelPriceSummary,
  type PriceSyncSettings,
  type PriceSyncResult,
} from "../services/api";

const F = { title: 15, body: 14, hint: 13, small: 12 } as const;
const PAGE_SIZE = 50;

export function PricingTab() {
  const { t } = useTranslation();
  const [prices, setPrices] = useState<ModelPriceSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [message, setMessage] = useState("");
  const [syncing, setSyncing] = useState(false);
  const [syncSettings, setSyncSettings] = useState<PriceSyncSettings>({
    auto_sync_enabled: false,
    sync_interval_secs: 86400,
    last_sync_at: 0,
    fallback_input_price: 3.0,
    fallback_output_price: 3.0,
  });

  const loadSettings = useCallback(async () => {
    try {
      const s = await priceSyncApi.get();
      setSyncSettings(s);
    } catch { /* defaults */ }
  }, []);

  const load = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
    try {
      if (searchQuery.trim()) {
        const items = await modelPriceApi.search(searchQuery.trim(), PAGE_SIZE);
        setPrices(items || []);
        setTotal(items?.length ?? 0);
      } else {
        const [items, count] = await Promise.all([
          modelPriceApi.list(PAGE_SIZE, offset),
          modelPriceApi.count(),
        ]);
        setPrices(items || []);
        setTotal(count);
      }
    } catch (e) { console.error(e); }
    if (!silent) setLoading(false);
  }, [offset, searchQuery]);

  useEffect(() => { loadSettings(); }, [loadSettings]);
  useEffect(() => { load(); }, [load]);

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

  const handleDelete = async (modelName: string) => {
    try {
      await modelPriceApi.delete(modelName);
      load();
    } catch (e: any) { setMessage(e.toString()); }
  };

  const totalPages = Math.ceil(total / PAGE_SIZE);
  const currentPage = Math.floor(offset / PAGE_SIZE) + 1;

  const formatPrice = (v: number | null) => {
    if (v == null) return "-";
    if (v < 0.01) return v.toFixed(4);
    if (v < 1) return v.toFixed(3);
    return v.toFixed(2);
  };

  const formatTime = (ts: number) => {
    if (!ts) return "-";
    return new Date(ts).toLocaleString();
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, maxWidth: 800, width: "100%" }}>
      {/* Sync controls */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div style={{ fontSize: F.body, fontWeight: 600 }}>{t("pricing.syncTitle", "价格同步")}</div>
            <div className="text-secondary" style={{ fontSize: F.small, marginTop: 2 }}>
              {t("pricing.syncDesc", "从 LiteLLM 同步模型价格表（含各平台价格）")}
            </div>
          </div>
          <button className="btn" onClick={handleSync} disabled={syncing} style={{ fontSize: F.hint }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              {syncing ? t("pricing.syncing", "同步中...") : t("pricing.syncNow", "立即同步")}
              {syncing && (
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="spin">
                  <path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" />
                </svg>
              )}
            </span>
          </button>
        </div>

        {/* Sync settings row */}
        <div style={{ display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap", paddingTop: 8, borderTop: "1px solid var(--border)" }}>
          {/* Auto sync toggle */}
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

          {/* Sync interval */}
          {syncSettings.auto_sync_enabled && (
            <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
              <label style={{ fontSize: F.small, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                {t("pricing.interval", "间隔")}
              </label>
              <select
                className="input"
                value={syncSettings.sync_interval_secs}
                onChange={(e) => updateSettings({ sync_interval_secs: Number(e.target.value) })}
                style={{ padding: "3px 6px", fontSize: F.small, width: 100 }}
              >
                <option value={3600}>1h</option>
                <option value={21600}>6h</option>
                <option value={43200}>12h</option>
                <option value={86400}>24h</option>
                <option value={604800}>7d</option>
              </select>
            </div>
          )}

          {/* Last sync time */}
          <span style={{ fontSize: F.small, color: "var(--text-tertiary)", marginLeft: "auto" }}>
            {t("pricing.lastSync", "上次同步")}: {formatTime(syncSettings.last_sync_at)}
          </span>
        </div>

        {/* Fallback price */}
        <div style={{ display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap", paddingTop: 8, borderTop: "1px solid var(--border)" }}>
          <span style={{ fontSize: F.small, fontWeight: 600 }}>{t("pricing.fallback", "兜底价格 $/M tokens")}</span>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <label style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{t("pricing.input", "输入")}</label>
            <input
              className="input" type="number" min={0} step={0.1}
              value={syncSettings.fallback_input_price}
              onChange={(e) => updateSettings({ fallback_input_price: Math.max(0, Number(e.target.value)) })}
              style={{ width: 70, padding: "3px 6px", fontSize: F.small }}
            />
          </div>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <label style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{t("pricing.output", "输出")}</label>
            <input
              className="input" type="number" min={0} step={0.1}
              value={syncSettings.fallback_output_price}
              onChange={(e) => updateSettings({ fallback_output_price: Math.max(0, Number(e.target.value)) })}
              style={{ width: 70, padding: "3px 6px", fontSize: F.small }}
            />
          </div>
        </div>
      </div>

      {/* Search */}
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <input
          className="input"
          placeholder={t("pricing.searchPlaceholder", "搜索模型名称...")}
          value={searchQuery}
          onChange={(e) => { setSearchQuery(e.target.value); setOffset(0); }}
          style={{ flex: 1, fontSize: F.hint, padding: "8px 12px" }}
        />
        <button className="btn" onClick={() => load()} disabled={loading} style={{ fontSize: F.hint }}>
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" /></svg>
        </button>
      </div>

      {/* Price table */}
      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading")}</div>
      ) : prices.length === 0 ? (
        <div className="glass-surface" style={{ padding: 40, textAlign: "center" }}>
          <div className="text-tertiary" style={{ fontSize: F.hint }}>
            {t("pricing.empty", "暂无价格数据，请点击「立即同步」从 LiteLLM 获取")}
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
                  <ThCell>{""}</ThCell>
                </tr>
              </thead>
              <tbody>
                {prices.map((p) => (
                  <tr key={p.id} style={{ borderBottom: "1px solid var(--border)" }}>
                    <TdCell><span style={{ fontWeight: 500, fontSize: F.small }}>{p.model_name}</span></TdCell>
                    <TdCell>
                      <span className="badge" style={{
                        fontSize: 10,
                        padding: "1px 6px",
                        background: p.source === "manual" ? "var(--accent-dim, rgba(0,122,255,0.1))" : "var(--bg-secondary)",
                        color: p.source === "manual" ? "var(--accent)" : "var(--text-secondary)",
                      }}>
                        {p.source}
                      </span>
                    </TdCell>
                    <TdCell><span style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{p.default_platform || "-"}</span></TdCell>
                    <TdCell>{formatPrice(p.input_price)}</TdCell>
                    <TdCell>{formatPrice(p.output_price)}</TdCell>
                    <TdCell>{formatPrice(p.cache_read_price)}</TdCell>
                    <TdCell>
                      <button
                        className="btn btn-ghost btn-icon"
                        style={{ padding: 2 }}
                        title={t("pricing.delete", "删除")}
                        onClick={() => handleDelete(p.model_name)}
                      >
                        <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="var(--color-danger, #ff3b30)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 2l10 10M12 2L2 12" /></svg>
                      </button>
                    </TdCell>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          {totalPages > 1 && !searchQuery && (
            <div style={{ display: "flex", justifyContent: "center", alignItems: "center", gap: 12 }}>
              <button className="btn" disabled={currentPage <= 1}
                onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>←</button>
              <span className="text-secondary" style={{ fontSize: F.hint }}>
                {currentPage} / {totalPages}
              </span>
              <button className="btn" disabled={currentPage >= totalPages}
                onClick={() => setOffset(offset + PAGE_SIZE)}>→</button>
            </div>
          )}

          <div className="text-tertiary" style={{ fontSize: F.small, textAlign: "center" }}>
            {t("pricing.total", "共 {count} 个模型价格").replace("{count}", String(total))}
          </div>
        </>
      )}

      {message && <div className="toast">{message}</div>}
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
