import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { proxyApi, proxyLogApi, proxyTimeoutApi, appLogApi, type ProxyLogSettings, type AppLogSettings } from "../services/api";
import { Settings } from "./Settings";
import { CodexSettings } from "./CodexSettings";
import { PricingTab } from "./PricingTab";
import { TrayConfigTab } from "./TrayConfigTab";
import { requestNavigation } from "../utils/navGuard";

type Tab = "system" | "claude" | "codex" | "pricing" | "tray";

export function AppSettings({ onLogSettingsChanged }: { onLogSettingsChanged?: (enabled: boolean) => void }) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("system");
  // Switching tabs may be intercepted by a dirty page (e.g. Claude Code Settings).
  const switchTab = (next: Tab) => {
    if (next === tab) return;
    requestNavigation(() => setTab(next));
  };
  const [running, setRunning] = useState(false);
  const [proxyPort, setProxyPort] = useState(9876);
  const [autostart, setAutostart] = useState(false);
  const [logEnabled, setLogEnabled] = useState(false);
  const [logRetention, setLogRetention] = useState(90);
  const [logUserReq, setLogUserReq] = useState(true);
  const [logUpstreamReq, setLogUpstreamReq] = useState(true);
  const [userReqRetention, setUserReqRetention] = useState(7);
  const [upstreamReqRetention, setUpstreamReqRetention] = useState(7);
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
        setProxyPort(s.port);
      } catch { /* defaults */ }
      try {
        const s = await proxyApi.status();
        setRunning(s);
      } catch { setRunning(false); }
      try {
        const ls = await proxyLogApi.getSettings();
        setLogEnabled(ls.enabled);
        setLogRetention(ls.retention_days);
        setLogUserReq(ls.log_user_request);
        setLogUpstreamReq(ls.log_upstream_request);
        setUserReqRetention(ls.user_request_retention_days);
        setUpstreamReqRetention(ls.upstream_request_retention_days);
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

  const handleProxyStart = async () => {
    try {
      const msg = await proxyApi.start(proxyPort);
      setRunning(true);
      setMessage(msg);
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleProxyStop = async () => {
    try {
      await proxyApi.stop();
      setRunning(false);
      setMessage(t("proxy.stopped"));
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleAutostartChange = async (val: boolean) => {
    try {
      await proxyApi.setAutostart(val);
      setAutostart(val);
    } catch (e: any) { setMessage(e.toString()); }
  };

  const buildLogSettings = (): ProxyLogSettings => ({
    enabled: logEnabled,
    log_user_request: logUserReq,
    log_upstream_request: logUpstreamReq,
    user_request_retention_days: userReqRetention,
    upstream_request_retention_days: upstreamReqRetention,
    retention_days: logRetention,
  });

  const handleLogEnabledChange = async (val: boolean) => {
    try {
      const settings: ProxyLogSettings = { ...buildLogSettings(), enabled: val };
      await proxyLogApi.setSettings(settings);
      setLogEnabled(val);
      onLogSettingsChanged?.(val);
    } catch (e: any) { setMessage(e.toString()); }
  };

  const updateLogSettings = async (partial: Partial<ProxyLogSettings>) => {
    const settings = { ...buildLogSettings(), ...partial };
    try {
      await proxyLogApi.setSettings(settings);
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleTimeoutChange = async (req: number, conn: number) => {
    setReqTimeout(req);
    setConnTimeout(conn);
    try {
      await proxyTimeoutApi.set({ request_timeout_secs: req, connect_timeout_secs: conn, source_protocol: "anthropic" });
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
        {(["system", "claude", "codex", "pricing", "tray"] as Tab[]).map((id) => (
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
            onClick={() => switchTab(id)}
          >
            {id === "system"
              ? t("appSettings.systemTab", "系统设置")
              : id === "claude"
                ? t("appSettings.claudeTab", "Claude Code")
                : id === "codex"
                  ? t("appSettings.codexTab", "Codex")
                  : id === "pricing"
                    ? t("appSettings.pricingTab", "模型价格")
                    : t("appSettings.trayTab", "系统托盘")}
          </button>
        ))}
      </div>

      {tab === "pricing" ? (
        <PricingTab />
      ) : tab === "tray" ? (
        <TrayConfigTab />
      ) : tab === "system" ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
          {/* Proxy Status */}
          <div
            className={`glass glass-highlight ${running ? "" : ""}`}
            style={{
              padding: "24px 20px",
              display: "flex",
              alignItems: "center",
              gap: 20,
              ...(running ? { animation: running ? "pulseGlow 3s ease-in-out infinite" : undefined } : {}),
            }}
          >
            <div style={{
              width: 44, height: 44, borderRadius: 22,
              flexShrink: 0,
              display: "flex", alignItems: "center", justifyContent: "center",
              background: running
                ? "linear-gradient(135deg, rgba(52,199,89,0.2), rgba(52,199,89,0.05))"
                : "var(--bg-glass)",
              border: `1px solid ${running ? "rgba(52,199,89,0.2)" : "var(--border)"}`,
              transition: "all 400ms ease",
            }}>
              <span className={`status-dot ${running ? "status-dot-active" : "status-dot-inactive"}`}
                style={{ width: 16, height: 16 }} />
            </div>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 14, fontWeight: 700 }}>
                {running ? t("proxy.running") : t("proxy.stopped")}
              </div>
              {running && (
                <div style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 2 }}>
                  localhost:{proxyPort}
                </div>
              )}
            </div>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                {t("proxy.port")}
              </label>
              <input
                className="input"
                type="number"
                value={proxyPort}
                onChange={(e) => setProxyPort(Number(e.target.value))}
                disabled={running}
                style={{ width: 80 }}
              />
              {!running ? (
                <button className="btn btn-primary" onClick={handleProxyStart}>
                  {t("proxy.start")}
                </button>
              ) : (
                <button className="btn btn-danger" onClick={handleProxyStop}>
                  {t("proxy.stop")}
                </button>
              )}
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
              <>
                {/* Sub-toggles for recording scope */}
                <div style={{ paddingTop: 8, borderTop: "1px solid var(--border)", display: "flex", flexDirection: "column", gap: 10 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 600 }}>{t("proxy.logUserReq", "记录用户原始请求")}</div>
                      <div className="text-tertiary" style={{ fontSize: 11, marginTop: 1 }}>
                        {t("proxy.logUserReqDesc", "用户发送的请求头和请求体")}
                      </div>
                    </div>
                    <div
                      className={`toggle ${logUserReq ? "active" : ""}`}
                      onClick={() => { setLogUserReq(!logUserReq); updateLogSettings({ log_user_request: !logUserReq }); }}
                      role="switch"
                      aria-checked={logUserReq}
                      tabIndex={0}
                    />
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 600 }}>{t("proxy.logUpstreamReq", "记录实际上游请求")}</div>
                      <div className="text-tertiary" style={{ fontSize: 11, marginTop: 1 }}>
                        {t("proxy.logUpstreamReqDesc", "发送到上游平台的请求头和请求体")}
                      </div>
                    </div>
                    <div
                      className={`toggle ${logUpstreamReq ? "active" : ""}`}
                      onClick={() => { setLogUpstreamReq(!logUpstreamReq); updateLogSettings({ log_upstream_request: !logUpstreamReq }); }}
                      role="switch"
                      aria-checked={logUpstreamReq}
                      tabIndex={0}
                    />
                  </div>
                </div>

                {/* Retention settings */}
                <div style={{ paddingTop: 8, borderTop: "1px solid var(--border)", display: "flex", flexDirection: "column", gap: 8 }}>
                  {logUserReq && (
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap", minWidth: 120 }}>
                        {t("proxy.userReqRetention", "原始请求保留天数")}
                      </label>
                      <input
                        className="input"
                        type="number"
                        min={0}
                        value={userReqRetention}
                        onChange={(e) => { const v = Math.max(0, Number(e.target.value)); setUserReqRetention(v); updateLogSettings({ user_request_retention_days: v }); }}
                        style={{ width: 70 }}
                      />
                      <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                        {userReqRetention === 0 ? t("proxy.logRetentionForever", "永久保留") : t("unit.days", "天")}
                      </span>
                    </div>
                  )}
                  {logUpstreamReq && (
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap", minWidth: 120 }}>
                        {t("proxy.upstreamReqRetention", "上游请求保留天数")}
                      </label>
                      <input
                        className="input"
                        type="number"
                        min={0}
                        value={upstreamReqRetention}
                        onChange={(e) => { const v = Math.max(0, Number(e.target.value)); setUpstreamReqRetention(v); updateLogSettings({ upstream_request_retention_days: v }); }}
                        style={{ width: 70 }}
                      />
                      <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                        {upstreamReqRetention === 0 ? t("proxy.logRetentionForever", "永久保留") : t("unit.days", "天")}
                      </span>
                    </div>
                  )}
                  <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap", minWidth: 120 }}>
                      {t("proxy.logRetention", "日志记录保留天数")}
                    </label>
                    <input
                      className="input"
                      type="number"
                      min={0}
                      value={logRetention}
                      onChange={(e) => { const v = Math.max(0, Number(e.target.value)); setLogRetention(v); updateLogSettings({ retention_days: v }); }}
                      style={{ width: 70 }}
                    />
                    <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                      {logRetention === 0 ? t("proxy.logRetentionForever", "永久保留") : t("unit.days", "天")}
                    </span>
                  </div>
                </div>
              </>
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
                  {t("appLog.desc", "控制台日志始终输出；以下设置仅影响日志文件")}
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
      ) : tab === "codex" ? (
        <CodexSettings />
      ) : (
        <Settings />
      )}
    </div>
  );
}
