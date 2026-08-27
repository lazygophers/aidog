// 同步状态区：上次同步时间 / 自动同步开关 / 兜底价格 / 手动触发 + partial 失败清单（T3 交付的 failures）。

import { useTranslation } from "react-i18next";
import type { PriceSyncResult, PriceSyncSettings } from "../../services/api";
import { F } from "../../domains/shared/tokens";
import { formatDateTime } from "../../utils/formatters";
import { makeRipple, useReveal } from "../../components/shared";
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

export function SyncStatusCard({
  settings, onUpdateSettings, syncing, onSync, result, bundled,
}: {
  settings: PriceSyncSettings;
  onUpdateSettings: (partial: Partial<PriceSyncSettings>) => void;
  syncing: boolean;
  onSync: () => void;
  /** 最近一次手动同步结果；null = 本次会话尚未同步。 */
  result: PriceSyncResult | null;
  /** true = 当前展示的是编译期内置 registry 兜底（DB 尚无同步数据）。 */
  bundled: boolean;
}) {
  const { t } = useTranslation();
  const card = useReveal<HTMLDivElement>(0);
  const lastSync = settings.last_sync_at ? formatDateTime(settings.last_sync_at) : null;
  const failures = result?.failures ?? [];

  return (
    <div
      ref={card.ref}
      className={`glass-surface hover-lift reveal${card.shown ? " in" : ""}`}
      style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}>
        <div>
          <div style={{ fontSize: F.body, fontWeight: 600 }}>{t("modelInfo.syncTitle")}</div>
          <div className="text-secondary" style={{ fontSize: F.small, marginTop: 2 }}>
            {t("modelInfo.syncDesc")}
          </div>
        </div>
        <Button
          className="ripple"
          onClick={(e) => { makeRipple(e); onSync(); }}
          disabled={syncing}
          style={{ fontSize: F.hint, height: "auto", padding: "4px 10px" }}
        >
          <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
            {syncing ? t("modelInfo.syncing") : t("modelInfo.syncNow")}
            {syncing && (
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="spin">
                <path d="M1.5 7a5.5 5.5 0 1 1 1.3 3.6M1.5 11V7.5H5" />
              </svg>
            )}
          </span>
        </Button>
      </div>

      {bundled && (
        <div className="text-secondary" style={{ fontSize: F.small }}>
          {t("modelInfo.bundledNotice")}
        </div>
      )}

      <div style={{ display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap", paddingTop: 8, borderTop: "1px solid color-mix(in srgb, var(--border) 45%, transparent)" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <Switch
            checked={settings.auto_sync_enabled}
            onCheckedChange={(v) => onUpdateSettings({ auto_sync_enabled: v })}
          />
          <span style={{ fontSize: F.small, fontWeight: 600 }}>{t("modelInfo.autoSync")}</span>
        </div>
        {settings.auto_sync_enabled && (
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <label style={{ fontSize: F.small, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("modelInfo.interval")}
            </label>
            <Select
              value={String(settings.sync_interval_secs)}
              onValueChange={(v) => onUpdateSettings({ sync_interval_secs: Number(v) })}
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
          {t("modelInfo.lastSync")}: {lastSync ?? "-"}
        </span>
      </div>

      <div style={{ display: "flex", gap: 16, alignItems: "center", flexWrap: "wrap", paddingTop: 8, borderTop: "1px solid color-mix(in srgb, var(--border) 45%, transparent)" }}>
        <span style={{ fontSize: F.small, fontWeight: 600 }}>{t("modelInfo.fallback")}</span>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <label style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{t("modelInfo.input")}</label>
          <Input
            type="number" min={0} step={0.1}
            value={settings.fallback_input_price}
            onChange={(e) => onUpdateSettings({ fallback_input_price: Math.max(0, Number(e.target.value)) })}
            style={{ width: 70, padding: "3px 6px", fontSize: F.small, height: 30 }}
          />
        </div>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <label style={{ fontSize: F.small, color: "var(--text-secondary)" }}>{t("modelInfo.output")}</label>
          <Input
            type="number" min={0} step={0.1}
            value={settings.fallback_output_price}
            onChange={(e) => onUpdateSettings({ fallback_output_price: Math.max(0, Number(e.target.value)) })}
            style={{ width: 70, padding: "3px 6px", fontSize: F.small, height: 30 }}
          />
        </div>
      </div>

      {result && (
        <div style={{ paddingTop: 8, borderTop: "1px solid color-mix(in srgb, var(--border) 45%, transparent)", display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: F.small }}>
            {t("modelInfo.syncResult")
              .replace("{added}", String(result.added))
              .replace("{updated}", String(result.updated))
              .replace("{failed}", String(result.failed))
              .replace("{total}", String(result.total))}
          </div>
          {failures.length > 0 && (
            <details open>
              <summary style={{ fontSize: F.small, color: "var(--danger)", cursor: "pointer" }}>
                {t("modelInfo.failuresTitle").replace("{count}", String(failures.length))}
              </summary>
              <ul style={{ margin: "6px 0 0", paddingInlineStart: 18, maxHeight: 160, overflow: "auto" }}>
                {failures.map(f => (
                  <li key={f.file} style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>
                    <code>{f.file}</code> — {f.error}
                  </li>
                ))}
              </ul>
            </details>
          )}
        </div>
      )}
    </div>
  );
}
