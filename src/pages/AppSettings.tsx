import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { proxyApi, proxyLogApi, type ProxyLogSettings } from "../services/api";

export function AppSettings({ onLogSettingsChanged }: { onLogSettingsChanged?: (enabled: boolean) => void }) {
  const { t } = useTranslation();
  const [autostart, setAutostart] = useState(false);
  const [logEnabled, setLogEnabled] = useState(false);
  const [logRetention, setLogRetention] = useState(7);
  const [message, setMessage] = useState("");

  useEffect(() => {
    (async () => {
      try {
        const s = await proxyApi.getSettings();
        setAutostart(s.autostart);
      } catch { /* defaults */ }
      try {
        const ls = await proxyLogApi.getSettings();
        setLogEnabled(ls.enabled);
        setLogRetention(ls.retention_days);
      } catch { /* defaults */ }
    })();
  }, []);

  const handleAutostartChange = async (val: boolean) => {
    try {
      await proxyApi.setAutostart(val);
      setAutostart(val);
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleLogEnabledChange = async (val: boolean) => {
    try {
      const settings: ProxyLogSettings = { enabled: val, retention_days: logRetention };
      await proxyLogApi.setSettings(settings);
      setLogEnabled(val);
      onLogSettingsChanged?.(val);
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleLogRetentionChange = async (days: number) => {
    setLogRetention(days);
    try {
      const settings: ProxyLogSettings = { enabled: logEnabled, retention_days: days };
      await proxyLogApi.setSettings(settings);
    } catch (e: any) { setMessage(e.toString()); }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 640, width: "100%" }}>
      {/* Header */}
      <div className="section-header">
        <div>
          <div className="section-title">{t("page.appSettings", "应用设置")}</div>
          <div className="section-desc">{t("appSettings.desc", "系统级代理配置")}</div>
        </div>
      </div>

      {/* Autostart */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.autostart")}</div>
          <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
            {t("proxy.autostartDesc")}
          </div>
        </div>
        <div
          className={`toggle ${autostart ? "active" : ""}`}
          onClick={() => handleAutostartChange(!autostart)}
          role="switch"
          aria-checked={autostart}
          tabIndex={0}
        />
      </div>

      {/* Log recording */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        flexDirection: "column",
        gap: 12,
      }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.logRequests", "记录请求日志")}</div>
            <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
              {t("proxy.logRequestsDesc", "记录代理请求的头部、内容、耗时和 Token 消耗")}
            </div>
          </div>
          <div
            className={`toggle ${logEnabled ? "active" : ""}`}
            onClick={() => handleLogEnabledChange(!logEnabled)}
            role="switch"
            aria-checked={logEnabled}
            tabIndex={0}
          />
        </div>

        {logEnabled && (
          <div style={{ display: "flex", gap: 12, alignItems: "center", paddingTop: 8, borderTop: "1px solid var(--border)" }}>
            <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
              {t("proxy.logRetention", "保留天数")}
            </label>
            <input
              className="input"
              type="number"
              min={0}
              value={logRetention}
              onChange={(e) => handleLogRetentionChange(Math.max(0, Number(e.target.value)))}
              style={{ width: 80 }}
            />
            <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
              {logRetention === 0 ? t("proxy.logRetentionForever", "永久保留") : t("proxy.logRetentionHint", "0 = 永久保留")}
            </span>
          </div>
        )}
      </div>

      {message && <div className="toast">{message}</div>}
    </div>
  );
}
