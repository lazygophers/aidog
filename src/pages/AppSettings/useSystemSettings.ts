import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { proxyApi, proxyLogApi, proxyTimeoutApi, appLogApi, dbApi, statsApi, statsSettingsApi, autoUpdateApi, type ProxyLogSettings, type AppLogSettings, type ProxyClientSettings } from "../../services/api";

/**
 * system tab 全部 state + actions（AppSettings 拆分自原 L21-240）。
 * 23 useState + 3 useEffect + 13 handler 原样外迁，无逻辑变更。
 */
export function useSystemSettings(onLogSettingsChanged?: (enabled: boolean) => void) {
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
  const [autoUpdateEnabled, setAutoUpdateEnabled] = useState(true);
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
      try {
        const au = await autoUpdateApi.get();
        setAutoUpdateEnabled(au);
      } catch { /* defaults: keep true */ }
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

  const handleAutoUpdateChange = async (val: boolean) => {
    try {
      await autoUpdateApi.set(val);
      setAutoUpdateEnabled(val);
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

  return {
    // state
    running, proxyPort, autostart, bindLan, autolaunch, silentLaunch,
    logEnabled, logRetention, logUserReq, logUpstreamReq,
    userReqRetention, upstreamReqRetention, reqTimeout, connTimeout,
    logFileEnabled, logLevel, logRetHours, message, appVersion,
    dbCompacting, statsRetention, statsRebuilding, proxyClient,
    autoUpdateEnabled,
    // state setters needed directly in JSX (inline onChange)
    setProxyPort, setLogUserReq, setLogUpstreamReq,
    setUserReqRetention, setUpstreamReqRetention, setLogRetention,
    // actions
    handleProxyStart, handleProxyStop,
    handleAutostartChange, handleBindLanChange,
    handleAutolaunchChange, handleSilentLaunchChange,
    handleProxyClientChange, handleLogEnabledChange, updateLogSettings,
    handleDbCompact, handleStatsRetentionChange, handleStatsRebuild,
    handleTimeoutChange, handleLogSettingsChange, handleAutoUpdateChange,
  };
}

export type SystemSettings = ReturnType<typeof useSystemSettings>;
