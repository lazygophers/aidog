import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { proxyApi, proxyLogApi, proxyTimeoutApi, appLogApi, type ProxyLogSettings, type AppLogSettings } from "../services/api";
import { Settings } from "./Settings";

type Tab = "proxy" | "claude";

export function AppSettings({ onLogSettingsChanged }: { onLogSettingsChanged?: (enabled: boolean) => void }) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("proxy");
  const [autostart, setAutostart] = useState(false);
  const [logEnabled, setLogEnabled] = useState(false);
  const [logRetention, setLogRetention] = useState(7);
  const [reqTimeout, setReqTimeout] = useState(300);
  const [connTimeout, setConnTimeout] = useState(10);
  const [logFileEnabled, setLogFileEnabled] = useState(true);
  const [logLevel, setLogLevel] = useState("info");
  const [logRetHours, setLogRetHours] = useState(3);
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
      try {
        const ts = await proxyTimeoutApi.get();
        setReqTimeout(ts.request_timeout_secs);
        setConnTimeout(ts.connect_timeout_secs);
      } catch { /* defaults */ }
      try {
        const ls = await appLogApi.get();
        setLogFileEnabled(ls.file_enabled);
        setLogLevel(ls.level);
        setLogRetHours(ls.retention_hours);
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

  const handleTimeoutChange = async (req: number, conn: number) => {
    setReqTimeout(req);
    setConnTimeout(conn);
    try {
      await proxyTimeoutApi.set({ request_timeout_secs: req, connect_timeout_secs: conn });
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleLogSettingsChange = async (partial: Partial<AppLogSettings>) => {
    const next = { file_enabled: logFileEnabled, level: logLevel, retention_hours: logRetHours, ...partial };
    setLogFileEnabled(next.file_enabled);
    setLogLevel(next.level);
    setLogRetHours(next.retention_hours);
    try {
      await appLogApi.set(next);
    } catch (e: any) { setMessage(e.toString()); }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* Tab bar */}
      <div style={{ display: "flex", gap: 0, borderBottom: "1px solid var(--border)" }}>
        {(["proxy", "claude"] as Tab[]).map((id) => (
          <button
            key={id}
            className="btn btn-ghost"
            style={{
              padding: "10px 16px",
              fontSize: 13,
              fontWeight: tab === id ? 600 : 400,
              color: tab === id ? "var(--accent)" : "var(--text-secondary)",
              borderBottom: tab === id ? "2px solid var(--accent)" : "2px solid transparent",
              borderRadius: 0,
            }}
            onClick={() => setTab(id)}
          >
            {id === "proxy"
              ? t("appSettings.proxyTab", "代理配置")
              : t("appSettings.claudeTab", "Claude Code")}
          </button>
        ))}
      </div>

      {tab === "proxy" ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 640 }}>
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
                <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>s</span>
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
                <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>s</span>
              </div>
            </div>
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

          {/* Application Logging */}
          <div className="glass-surface" style={{
            padding: "16px 20px",
            display: "flex",
            flexDirection: "column",
            gap: 12,
          }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <div style={{ fontSize: 13, fontWeight: 600 }}>{t("appLog.title", "应用日志")}</div>
                <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
                  {t("appLog.desc", "控制台日志始终输出；以下设置仅影响日志文件（发布版本生效）")}
                </div>
              </div>
              <div
                className={`toggle ${logFileEnabled ? "active" : ""}`}
                onClick={() => handleLogSettingsChange({ file_enabled: !logFileEnabled })}
                role="switch"
                aria-checked={logFileEnabled}
                tabIndex={0}
              />
            </div>

            {logFileEnabled && (
              <div style={{ display: "flex", gap: 16, alignItems: "center", paddingTop: 8, borderTop: "1px solid var(--border)", flexWrap: "wrap" }}>
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                    {t("appLog.level", "日志级别")}
                  </label>
                  <select
                    className="input"
                    value={logLevel}
                    onChange={(e) => handleLogSettingsChange({ level: e.target.value })}
                    style={{ width: 90, padding: "4px 8px", fontSize: 12 }}
                  >
                    {["trace", "debug", "info", "warn", "error"].map((l) => (
                      <option key={l} value={l}>{l.toUpperCase()}</option>
                    ))}
                  </select>
                </div>
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                    {t("appLog.retention", "保留时长")}
                  </label>
                  <input
                    className="input"
                    type="number"
                    min={0}
                    value={logRetHours}
                    onChange={(e) => handleLogSettingsChange({ retention_hours: Math.max(0, Number(e.target.value)) })}
                    style={{ width: 70, padding: "4px 8px", fontSize: 12 }}
                  />
                  <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
                    {t("appLog.retentionUnit", "小时")}
                  </span>
                </div>
                <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                  {logRetHours === 0 ? t("appLog.retentionForever", "永久保留") : ""}
                </span>
              </div>
            )}
          </div>

          {message && <div className="toast">{message}</div>}
        </div>
      ) : (
        <Settings />
      )}
    </div>
  );
}
