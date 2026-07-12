// 定时备份 section（独立功能）。
// 消费 services/api.ts backupApi：开关 / 间隔(自由小时+快捷预设) / 保留天数 /
// 状态展示(上次/下次/错误) / 立即备份一次。复用 import_export 加密容器 (.aidogx)，
// 落盘 ~/.aidog/backups/，超期自动清理。后端常驻 loop 实时读 settings 即时生效。
// 抽自原 ImportExport.tsx L1299-1525。

import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { backupApi, type BackupSettings, type BackupResult } from "../../../services/api";
import { SectionHeader } from "./primitives";
import { pad } from "../../../utils/formatters";

const INTERVAL_PRESETS: { labelKey: string; defaultLabel: string; hours: number }[] = [
  { labelKey: "settings.backup.preset1h", defaultLabel: "1h", hours: 1 },
  { labelKey: "settings.backup.preset6h", defaultLabel: "6h", hours: 6 },
  { labelKey: "settings.backup.preset12h", defaultLabel: "12h", hours: 12 },
  { labelKey: "settings.backup.presetDaily", defaultLabel: "每天", hours: 24 },
  { labelKey: "settings.backup.presetWeekly", defaultLabel: "每周", hours: 168 },
];

function formatBackupTime(ms: number, t: TFunction): string {
  if (!ms) return t("settings.backup.never", "从未");
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function ScheduledBackupSection() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<BackupSettings | null>(null);
  const [running, setRunning] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [lastResultPath, setLastResultPath] = useState<string | null>(null);

  // 初次加载。
  useEffect(() => {
    backupApi.get().then(setSettings).catch(() => setSettings(null));
  }, []);

  if (!settings) {
    return (
      <section className="glass" style={{ padding: 20 }}>
        <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>…</div>
      </section>
    );
  }

  const patch = async (next: Partial<BackupSettings>) => {
    const merged = { ...settings, ...next };
    setSettings(merged); // 乐观更新
    try {
      const saved = await backupApi.set(merged);
      setSettings(saved);
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    }
  };

  const runNow = async () => {
    setRunning(true);
    setMsg(null);
    try {
      const r: BackupResult = await backupApi.runNow();
      if (r.ok) {
        setMsg({ ok: true, text: t("settings.backup.success", "备份成功") });
        setLastResultPath(r.path ?? null);
        // 刷新 last_backup_at。
        const fresh = await backupApi.get();
        setSettings(fresh);
      } else {
        setMsg({ ok: false, text: r.error ?? t("settings.backup.failed", "备份失败") });
      }
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    } finally {
      setRunning(false);
    }
  };

  const nextAt = settings.enabled && settings.last_backup_at
    ? settings.last_backup_at + settings.interval_hours * 3600 * 1000
    : 0;

  return (
    <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 16 }}>
      <SectionHeader
        icon="backup"
        title={t("settings.backup.title", "定时备份")}
        desc={t("settings.backup.desc", "按设定间隔自动导出全部数据为加密 .aidogx，落盘 ~/.aidog/backups/，超期自动清理。")}
      />

      {/* 总开关 */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <span style={{ fontSize: 13, color: "var(--text-primary)", fontWeight: 500 }}>
          {t("settings.backup.enable", "启用定时备份")}
        </span>
        <label className="toggle-wrap" style={{ cursor: "pointer", display: "flex", alignItems: "center" }}>
          <input
            type="checkbox"
            checked={settings.enabled}
            onChange={(e) => patch({ enabled: e.target.checked })}
            style={{ display: "none" }}
          />
          <span className={`toggle ${settings.enabled ? "active" : ""}`} />
        </label>
      </div>

      {settings.enabled && (
        <>
          {/* 间隔 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {t("settings.backup.interval", "备份间隔（小时）")}
            </span>
            <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              <input
                type="number"
                min={1}
                value={settings.interval_hours}
                onChange={(e) => {
                  const v = Math.max(1, Math.floor(Number(e.target.value) || 1));
                  patch({ interval_hours: v });
                }}
                style={{
                  width: 90, padding: "6px 10px", fontSize: 13,
                  borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
                  background: "var(--bg-input)", color: "var(--text-primary)",
                }}
              />
              <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{t("settings.backup.hours", "小时")}</span>
              {INTERVAL_PRESETS.map((p) => (
                <button
                  key={p.hours}
                  onClick={() => patch({ interval_hours: p.hours })}
                  className="ie-chip"
                  style={{
                    padding: "4px 10px", fontSize: 12, cursor: "pointer",
                    borderRadius: "var(--radius-md)",
                    border: settings.interval_hours === p.hours
                      ? "1px solid var(--accent)"
                      : "1px solid var(--border-default)",
                    background: settings.interval_hours === p.hours
                      ? "var(--accent-subtle)"
                      : "transparent",
                    color: "var(--text-primary)",
                  }}
                >
                  {t(p.labelKey, p.defaultLabel)}
                </button>
              ))}
            </div>
          </div>

          {/* 保留天数 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {t("settings.backup.retention", "保留天数")}
            </span>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <input
                type="number"
                min={1}
                max={90}
                value={settings.retention_days}
                onChange={(e) => {
                  const v = Math.min(90, Math.max(1, Math.floor(Number(e.target.value) || 7)));
                  patch({ retention_days: v });
                }}
                style={{
                  width: 90, padding: "6px 10px", fontSize: 13,
                  borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
                  background: "var(--bg-input)", color: "var(--text-primary)",
                }}
              />
              <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{t("settings.backup.days", "天（1-90）")}</span>
            </div>
          </div>

          {/* 状态 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4, padding: "10px 12px", borderRadius: "var(--radius-md)", background: "var(--bg-subtle)", fontSize: 12, color: "var(--text-secondary)" }}>
            <div>{t("settings.backup.lastBackup", "上次备份")}: <span style={{ color: "var(--text-primary)" }}>{formatBackupTime(settings.last_backup_at, t)}</span></div>
            {nextAt > 0 && (
              <div>{t("settings.backup.nextBackup", "下次预计")}: <span style={{ color: "var(--text-primary)" }}>{formatBackupTime(nextAt, t)}</span></div>
            )}
            <div>{t("settings.backup.location", "备份位置")}: <code style={{ fontSize: 11, color: "var(--text-tertiary)" }}>~/.aidog/backups/</code></div>
            {settings.last_backup_error && (
              <div style={{ color: "var(--color-danger)" }}>
                {t("settings.backup.lastError", "上次错误")}: {settings.last_backup_error}
              </div>
            )}
          </div>

          {/* 立即备份 */}
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <button
              onClick={runNow}
              disabled={running}
              style={{
                padding: "7px 16px", fontSize: 13, cursor: running ? "not-allowed" : "pointer",
                borderRadius: "var(--radius-md)", border: "1px solid var(--accent)",
                background: "var(--accent)", color: "var(--text-on-accent, #fff)",
                opacity: running ? 0.6 : 1,
              }}
            >
              {running ? t("settings.backup.running", "备份中…") : t("settings.backup.runNow", "立即备份一次")}
            </button>
            {lastResultPath && (
              <button
                onClick={() => { revealItemInDir(lastResultPath).catch(() => {}); }}
                style={{
                  padding: "7px 14px", fontSize: 12, cursor: "pointer",
                  borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
                  background: "transparent", color: "var(--text-secondary)",
                }}
              >
                {t("settings.backup.reveal", "在文件夹显示")}
              </button>
            )}
          </div>
        </>
      )}

      {msg && (
        <div style={{
          padding: "8px 12px", fontSize: 12, borderRadius: "var(--radius-md)",
          color: msg.ok ? "var(--color-success)" : "var(--color-danger)",
          background: msg.ok ? "var(--color-success-bg)" : "var(--color-danger-bg)",
        }}>
          {msg.text}
        </div>
      )}
    </section>
  );
}
