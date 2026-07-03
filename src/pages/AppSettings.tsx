import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { proxyApi, proxyLogApi, proxyTimeoutApi, appLogApi, dbApi, statsApi, statsSettingsApi, type ProxyLogSettings, type AppLogSettings, type ProxyClientSettings } from "../services/api";
import { Settings } from "./Settings";
import { CodexSettings } from "./CodexSettings";
import { PricingTab } from "./PricingTab";
import { TrayConfigTab } from "./TrayConfigTab";
import { PopoverConfigTab } from "./PopoverConfigTab";
import { MiddlewareSettingsTab } from "../components/settings/MiddlewareRules";
import { SchedulingSettingsTab } from "../components/settings/SchedulingSettings";
import { NotificationSettingsTab } from "../components/settings/NotificationSettings";
import { ImportExportTab } from "../components/settings/ImportExport/ImportExportTab";
import { CodingToolsSettingsTab } from "../components/settings/CodingToolsSettings";

export type Tab = "system" | "claude" | "codex" | "coding_tools" | "middleware" | "scheduling" | "notifications" | "pricing" | "tray" | "popover" | "importexport";

export function AppSettings({ tab, onLogSettingsChanged, onNotifSettingsChanged }: { tab: Tab; onLogSettingsChanged?: (enabled: boolean) => void; onNotifSettingsChanged?: (enabled: boolean) => void }) {
  const { t } = useTranslation();
  const [running, setRunning] = useState(false);
  const [proxyPort, setProxyPort] = useState(9876);
  const [autostart, setAutostart] = useState(false);
  const [bindLan, setBindLan] = useState(true);
  const [autolaunch, setAutolaunch] = useState(false);
  const [silentLaunch, setSilentLaunch] = useState(false);
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
  const [appVersion, setAppVersion] = useState("");
  const [dbCompacting, setDbCompacting] = useState(false);
  const [statsRetention, setStatsRetention] = useState(365);
  const [statsRebuilding, setStatsRebuilding] = useState(false);
  const [proxyClient, setProxyClient] = useState<ProxyClientSettings>({
    enabled: false, proxy_type: "socks5", host: "127.0.0.1", port: 7890,
    username: "", password: "", dns_over_proxy: true,
  });

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion(""));
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const s = await proxyApi.getSettings();
        setAutostart(s.autostart);
        setSilentLaunch(s.silent_launch);
        setBindLan(s.bind_lan);
        setProxyPort(s.port);
      } catch { /* defaults */ }
      try {
        const s = await proxyApi.status();
        setRunning(s);
      } catch { setRunning(false); }
      try {
        const al = await proxyApi.getAutolaunch();
        setAutolaunch(al);
        // silentLaunch 仅在 autolaunch (开机自启) 生效时有意义; autolaunch off 时强制 false 并持久化
        if (!al) {
          setSilentLaunch(false);
          try { await proxyApi.setSilentLaunch(false); } catch { /* ignore */ }
        }
      } catch { /* defaults */ }
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
      try {
        const pc = await proxyApi.getProxyClientSettings();
        setProxyClient(pc);
      } catch { /* defaults */ }
      try {
        const ss = await statsSettingsApi.get();
        setStatsRetention(ss.retention_days);
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

  const handleBindLanChange = async (val: boolean) => {
    try {
      await proxyApi.setBindLan(val);
      setBindLan(val);
      // 后端会在代理运行时自动重启使新绑定地址生效。
      setMessage(t("proxy.bindLanApplied"));
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleAutolaunchChange = async (val: boolean) => {
    try {
      await proxyApi.setAutolaunch(val);
      setAutolaunch(val);
      // 关闭开机自启时, 同步关闭并持久化静默启动 (UI 也会随之隐藏)
      if (!val && silentLaunch) {
        try {
          await proxyApi.setSilentLaunch(false);
          setSilentLaunch(false);
        } catch { /* ignore */ }
      }
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleSilentLaunchChange = async (val: boolean) => {
    try {
      await proxyApi.setSilentLaunch(val);
      setSilentLaunch(val);
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleProxyClientChange = async (partial: Partial<ProxyClientSettings>) => {
    const next = { ...proxyClient, ...partial };
    setProxyClient(next);
    try {
      await proxyApi.setProxyClientSettings(next);
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

  const handleDbCompact = async () => {
    // 确认：全量 VACUUM 锁库期间代理请求短暂排队
    const ok = window.confirm(t("settings.dbCompactHint", "全量 VACUUM，期间代理请求将短暂排队"));
    if (!ok) return;
    setDbCompacting(true);
    try {
      const r = await dbApi.compact();
      const beforeMB = (r.before_bytes / 1024 / 1024).toFixed(1);
      const afterMB = (r.after_bytes / 1024 / 1024).toFixed(1);
      const pct = r.before_bytes > 0
        ? Math.round((1 - r.after_bytes / r.before_bytes) * 100)
        : 0;
      setMessage(t("settings.dbCompactDone", "{{before}} MB → {{after}} MB（省 {{pct}}%）", { before: beforeMB, after: afterMB, pct: String(pct) }));
    } catch (e: any) { setMessage(e.toString()); }
    finally { setDbCompacting(false); }
  };

  const handleStatsRetentionChange = async (v: number) => {
    setStatsRetention(v);
    try {
      await statsSettingsApi.set({ retention_days: v });
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleStatsRebuild = async () => {
    setStatsRebuilding(true);
    try {
      await statsApi.rebuildFromLogs();
      setMessage(t("stats.rebuildDone", "聚合统计已从日志重建"));
    } catch (e: any) { setMessage(e.toString()); }
    finally { setStatsRebuilding(false); }
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
      {tab === "pricing" ? (
        <PricingTab />
      ) : tab === "tray" ? (
        <TrayConfigTab />
      ) : tab === "popover" ? (
        <PopoverConfigTab />
      ) : tab === "middleware" ? (
        <MiddlewareSettingsTab />
      ) : tab === "scheduling" ? (
        <SchedulingSettingsTab />
      ) : tab === "notifications" ? (
        <NotificationSettingsTab onEnabledChanged={onNotifSettingsChanged} />
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
                ? "linear-gradient(135deg, color-mix(in srgb, var(--color-success) 20%, transparent), color-mix(in srgb, var(--color-success) 5%, transparent))"
                : "var(--bg-glass)",
              border: `1px solid ${running ? "color-mix(in srgb, var(--color-success) 20%, transparent)" : "var(--border)"}`,
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

          {/* Bind LAN — allow other devices on the local network to connect */}
          <div className="glass-surface" style={{
            padding: "16px 20px",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}>
            <div>
              <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.bindLan")}</div>
              <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
                {t("proxy.bindLanDesc")}
              </div>
            </div>
            <div
              className={`toggle ${bindLan ? "active" : ""}`}
              onClick={() => handleBindLanChange(!bindLan)}
              role="switch"
              aria-checked={bindLan}
              tabIndex={0}
            />
          </div>

          {/* Autolaunch — OS login auto start */}
          <div className="glass-surface" style={{
            padding: "16px 20px",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}>
            <div>
              <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.autolaunch")}</div>
              <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
                {t("proxy.autolaunchDesc")}
              </div>
            </div>
            <div
              className={`toggle ${autolaunch ? "active" : ""}`}
              onClick={() => handleAutolaunchChange(!autolaunch)}
              role="switch"
              aria-checked={autolaunch}
              tabIndex={0}
            />
          </div>

          {/* Silent Launch — start minimized to tray; 仅在 autolaunch (开机自启) 开启时展示 */}
          {autolaunch && (
          <div className="glass-surface" style={{
            padding: "16px 20px",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}>
            <div>
              <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.silentLaunch")}</div>
              <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
                {t("proxy.silentLaunchDesc")}
              </div>
            </div>
            <div
              className={`toggle ${silentLaunch ? "active" : ""}`}
              onClick={() => handleSilentLaunchChange(!silentLaunch)}
              role="switch"
              aria-checked={silentLaunch}
              tabIndex={0}
            />
          </div>
          )}

          {/* Upstream Proxy */}
          <div className="glass-surface" style={{
            padding: "16px 20px",
            display: "flex",
            flexDirection: "column",
            gap: 12,
          }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <div style={{ fontSize: 13, fontWeight: 600 }}>{t("proxy.upstreamProxy")}</div>
                <div className="text-secondary" style={{ fontSize: 12, marginTop: 2 }}>
                  {t("proxy.upstreamProxyDesc")}
                </div>
              </div>
              <div
                className={`toggle ${proxyClient.enabled ? "active" : ""}`}
                onClick={() => handleProxyClientChange({ enabled: !proxyClient.enabled })}
                role="switch"
                aria-checked={proxyClient.enabled}
                tabIndex={0}
              />
            </div>

            {proxyClient.enabled && (
              <div style={{ display: "flex", flexDirection: "column", gap: 10, paddingTop: 8, borderTop: "1px solid var(--border)" }}>
                <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
                  <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                      {t("proxy.proxyType", "协议")}
                    </label>
                    <select
                      className="input"
                      value={proxyClient.proxy_type}
                      onChange={(e) => handleProxyClientChange({ proxy_type: e.target.value })}
                      style={{ width: 90, padding: "4px 8px", fontSize: 12 }}
                    >
                      <option value="socks5">SOCKS5</option>
                      <option value="http">HTTP</option>
                      <option value="https">HTTPS</option>
                    </select>
                  </div>
                  <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                      {t("proxy.proxyHost", "地址")}
                    </label>
                    <input
                      className="input"
                      value={proxyClient.host}
                      onChange={(e) => handleProxyClientChange({ host: e.target.value })}
                      style={{ width: 120, padding: "4px 8px", fontSize: 12 }}
                    />
                  </div>
                  <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                      {t("proxy.proxyPort", "端口")}
                    </label>
                    <input
                      className="input"
                      type="number"
                      value={proxyClient.port}
                      onChange={(e) => handleProxyClientChange({ port: Number(e.target.value) || 7890 })}
                      style={{ width: 70, padding: "4px 8px", fontSize: 12 }}
                    />
                  </div>
                </div>
                <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
                  <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                      {t("proxy.proxyUser", "用户名")}
                    </label>
                    <input
                      className="input"
                      value={proxyClient.username}
                      onChange={(e) => handleProxyClientChange({ username: e.target.value })}
                      placeholder={t("proxy.proxyUserPlaceholder", "可选")}
                      style={{ width: 100, padding: "4px 8px", fontSize: 12 }}
                    />
                  </div>
                  <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <label style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
                      {t("proxy.proxyPass", "密码")}
                    </label>
                    <input
                      className="input"
                      type="password"
                      value={proxyClient.password}
                      onChange={(e) => handleProxyClientChange({ password: e.target.value })}
                      placeholder={t("proxy.proxyPassPlaceholder", "可选")}
                      style={{ width: 100, padding: "4px 8px", fontSize: 12 }}
                    />
                  </div>
                </div>
                {proxyClient.proxy_type === "socks5" && (
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 600 }}>{t("proxy.dnsOverProxy", "DNS 走代理")}</div>
                      <div className="text-tertiary" style={{ fontSize: 11, marginTop: 1 }}>
                        {t("proxy.dnsOverProxyDesc", "SOCKS5h: DNS 解析也走代理解析")}
                      </div>
                    </div>
                    <div
                      className={`toggle ${proxyClient.dns_over_proxy ? "active" : ""}`}
                      onClick={() => handleProxyClientChange({ dns_over_proxy: !proxyClient.dns_over_proxy })}
                      role="switch"
                      aria-checked={proxyClient.dns_over_proxy}
                      tabIndex={0}
                    />
                  </div>
                )}
              </div>
            )}
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
        </div>
      ) : tab === "codex" ? (
        <CodexSettings />
      ) : tab === "coding_tools" ? (
        <CodingToolsSettingsTab />
      ) : tab === "importexport" ? (
        <ImportExportTab />
      ) : (
        <Settings />
      )}
    </div>
  );
}
