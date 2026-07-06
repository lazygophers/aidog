import { useTranslation } from "react-i18next";
import { useEffect, useState } from "react";
import { getDefaultsJson, syncDefaultsJson } from "../../services/api";
import type { DefaultsSyncResult } from "../../services/api";
import { formatDateTime } from "../../utils/formatters";
import type { SystemSettings } from "./useSystemSettings";

/**
 * Timeout + DB Maintenance + Aggregate Stats + App version + toast（原 L519-560 + L680-755 + L820-837）。
 */
export function SystemMiscSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const { reqTimeout, connTimeout, handleTimeoutChange } = s;

  return (
    <>
      {/* Timeout */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        flexDirection: "column",
        gap: 12,
      }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.timeout", "超时设置")}</div>
        <div className="text-secondary" style={{ fontSize: 12 }}>
          {t("proxy.timeoutDesc", "系统默认超时，分组和模型级别可覆盖")}
        </div>
        <div style={{ display: "flex", gap: 16, alignItems: "center", marginTop: 4 }}>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.requestTimeout", "请求超时")}
            </label>
            <input
              className="input"
              type="number"
              min={0}
              value={reqTimeout}
              onChange={(e) => handleTimeoutChange(Math.max(0, Number(e.target.value)), connTimeout)}
              style={{ width: 80 }}
            />
            <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{t("unit.sec", "秒")}</span>
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.connectTimeout", "连接超时")}
            </label>
            <input
              className="input"
              type="number"
              min={0}
              value={connTimeout}
              onChange={(e) => handleTimeoutChange(reqTimeout, Math.max(0, Number(e.target.value)))}
              style={{ width: 80 }}
            />
            <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{t("unit.sec", "秒")}</span>
          </div>
        </div>
      </div>
    </>
  );
}

/**
 * DB Maintenance + Aggregate Stats（原 L680-755）。
 * 与 Timeout/App version 拆为两组以便顺序编排（Timeout 先, DB/Stats 中段, App version + toast 尾）。
 */
export function DbStatsSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const {
    dbCompacting, handleDbCompact,
    statsRetention, statsRebuilding, handleStatsRetentionChange, handleStatsRebuild,
  } = s;

  return (
    <>
      {/* DB Maintenance — 全量 VACUUM 压缩数据库（Tier 1 手动回收入口） */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("settings.dbCompact", "立即压缩数据库")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("settings.dbCompactHint", "全量 VACUUM，期间代理请求将短暂排队")}
          </div>
        </div>
        <button
          className="btn"
          onClick={handleDbCompact}
          disabled={dbCompacting}
          style={{
            padding: "7px 16px", fontSize: 13, cursor: dbCompacting ? "not-allowed" : "pointer",
            borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
            background: "transparent", color: "var(--text-primary)",
            opacity: dbCompacting ? 0.6 : 1,
          }}
        >
          {dbCompacting ? t("common.loading", "加载中…") : t("settings.dbCompact", "立即压缩数据库")}
        </button>
      </div>

      {/* Aggregate Stats — 聚合统计表保留与重建（与日志开关解耦：关日志也累计统计） */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        flexDirection: "column",
        gap: 12,
      }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("stats.aggSettings", "聚合统计")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("stats.aggSettingsHint", "使用统计独立累计，不受请求日志开关影响")}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap", minWidth: 120 }}>
            {t("stats.aggRetention", "统计保留天数")}
          </label>
          <input
            className="input"
            type="number"
            min={0}
            value={statsRetention}
            onChange={(e) => handleStatsRetentionChange(Math.max(0, Number(e.target.value)))}
            style={{ width: 70 }}
          />
          <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
            {statsRetention === 0 ? t("proxy.logRetentionForever", "永久保留") : t("unit.days", "天")}
          </span>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}>
          <div className="text-secondary" style={{ fontSize: 12 }}>
            {t("stats.rebuildHint", "从历史请求日志全量重建聚合统计表")}
          </div>
          <button
            className="btn"
            onClick={handleStatsRebuild}
            disabled={statsRebuilding}
            style={{
              padding: "7px 16px", fontSize: 13, cursor: statsRebuilding ? "not-allowed" : "pointer",
              borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
              background: "transparent", color: "var(--text-primary)",
              opacity: statsRebuilding ? 0.6 : 1, whiteSpace: "nowrap",
            }}
          >
            {statsRebuilding ? t("common.loading", "加载中…") : t("stats.rebuild", "从日志重建统计")}
          </button>
        </div>
      </div>
    </>
  );
}

/**
 * App version + toast（原 L820 + L822-837）。
 */
export function VersionToastSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const { message, appVersion } = s;

  return (
    <>
      {message && <div className="toast">{message}</div>}

      {/* App version — 只读展示, 单一事实源 = tauri.conf.json (经 getVersion API) */}
      {appVersion && (
        <div className="glass-surface" style={{
          padding: "16px 20px",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("app.version")}</div>
          <div style={{
            fontSize: 13,
            fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
            color: "var(--text-secondary)",
          }}>v{appVersion}</div>
        </div>
      )}
    </>
  );
}

/**
 * defaults.json 同步区：显示当前 last_updated + 「立即检查更新」按钮 + 同步结果反馈。
 * 手动触发无视节流。同步失败不破坏现有功能（reader 端自动回退 bundled）。
 */
export function DefaultsSyncSection() {
  const { t } = useTranslation();
  const [lastUpdated, setLastUpdated] = useState<number | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [result, setResult] = useState<DefaultsSyncResult | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    getDefaultsJson()
      .then((raw) => {
        if (!alive) return;
        try {
          const parsed = JSON.parse(raw) as { last_updated?: number };
          setLastUpdated(parsed.last_updated ?? null);
        } catch {
          setLastUpdated(null);
        }
      })
      .catch((e) => {
        console.error("[defaultsSync] getDefaultsJson failed:", e);
        if (alive) setLoadErr(String(e));
      });
    return () => { alive = false; };
  }, [result]);

  const handleSync = async () => {
    setSyncing(true);
    try {
      const r = await syncDefaultsJson();
      setResult(r);
    } catch (e) {
      console.error("[defaultsSync] syncDefaultsJson failed:", e);
      setResult({ updated: false, lastUpdated: 0, source: "local", error: String(e) });
    } finally {
      setSyncing(false);
    }
  };

  const feedback = result
    ? result.error
      ? t("settings.defaultsSyncFailed", { error: result.error })
      : result.updated
        ? t("settings.defaultsSyncUpdated", { source: result.source })
        : t("settings.defaultsSyncUpToDate")
    : null;

  return (
    <div className="glass-surface" style={{
      padding: "16px 20px",
      display: "flex",
      flexDirection: "column",
      gap: 12,
    }}>
      <div>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("settings.defaultsSync")}</div>
        <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
          {t("settings.defaultsSyncHint")}
        </div>
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}>
        <div className="text-secondary" style={{ fontSize: 12 }}>
          {t("settings.defaultsSyncLastUpdated")}{" "}
          {formatDateTime(lastUpdated ? lastUpdated * 1000 : null) ?? (loadErr ? t("settings.defaultsSyncUnknown") : "—")}
        </div>
        <button
          className="btn"
          onClick={handleSync}
          disabled={syncing}
          style={{
            padding: "7px 16px", fontSize: 13, cursor: syncing ? "not-allowed" : "pointer",
            borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
            background: "transparent", color: "var(--text-primary)",
            opacity: syncing ? 0.6 : 1, whiteSpace: "nowrap",
          }}
        >
          {syncing ? t("common.loading") : t("settings.defaultsSyncCheck")}
        </button>
      </div>
      {feedback && (
        <div className="text-secondary" style={{ fontSize: 12 }}>{feedback}</div>
      )}
    </div>
  );
}
