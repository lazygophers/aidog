import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { proxyApi } from "../services/api";

export function Proxy() {
  const { t } = useTranslation();
  const [running, setRunning] = useState(false);
  const [port, setPort] = useState(9876);
  const [autostart, setAutostart] = useState(false);
  const [message, setMessage] = useState("");

  const loadSettings = async () => {
    try {
      const s = await proxyApi.getSettings();
      setPort(s.port);
      setAutostart(s.autostart);
    } catch { /* defaults */ }
  };

  const checkStatus = async () => {
    try {
      const s = await proxyApi.status();
      setRunning(s);
    } catch { setRunning(false); }
  };

  useEffect(() => { loadSettings(); checkStatus(); }, []);

  const handleStart = async () => {
    try {
      const msg = await proxyApi.start(port);
      setRunning(true);
      setMessage(msg);
    } catch (e: any) { setMessage(e.toString()); }
  };

  const handleStop = async () => {
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

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 640, width: "100%" }}>
      {/* Header */}
      <div className="section-header">
        <div>
          <div className="section-title">{t("page.proxy")}</div>
          <div className="section-desc">{running
            ? `${t("proxy.listening")} localhost:${port}`
            : t("proxy.stopped")
          }</div>
        </div>
      </div>

      {/* Status Hero Card */}
      <div
        className={`glass glass-highlight ${running ? "" : ""}`}
        style={{
          padding: "32px 24px",
          textAlign: "center",
          ...(running ? { animation: running ? "pulseGlow 3s ease-in-out infinite" : undefined } : {}),
        }}
      >
        <div style={{
          width: 56, height: 56, borderRadius: 28,
          margin: "0 auto 16px",
          display: "flex", alignItems: "center", justifyContent: "center",
          background: running
            ? "linear-gradient(135deg, rgba(52,199,89,0.2), rgba(52,199,89,0.05))"
            : "var(--bg-glass)",
          border: `1px solid ${running ? "rgba(52,199,89,0.2)" : "var(--border)"}`,
          transition: "all 400ms ease",
        }}>
          <span className={`status-dot ${running ? "status-dot-active" : "status-dot-inactive"}`}
            style={{ width: 20, height: 20 }} />
        </div>
        <div style={{ fontSize: 17, fontWeight: 700, marginBottom: 4, letterSpacing: "-0.01em" }}>
          {running ? t("proxy.running") : t("proxy.stopped")}
        </div>
        {running && (
          <div className="badge badge-accent" style={{ margin: "0 auto" }}>
            localhost:{port}
          </div>
        )}
      </div>

      {/* Controls Row */}
      <div className="glass-surface" style={{
        padding: "16px 20px",
        display: "flex",
        gap: 12,
        alignItems: "center",
      }}>
        <label style={{ fontSize: 13, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
          {t("proxy.port")}
        </label>
        <input
          className="input"
          type="number"
          value={port}
          onChange={(e) => setPort(Number(e.target.value))}
          disabled={running}
          style={{ width: 100 }}
        />
        <div style={{ flex: 1 }} />
        {!running ? (
          <button className="btn btn-primary" onClick={handleStart}>
            {t("proxy.start")}
          </button>
        ) : (
          <button className="btn btn-danger" onClick={handleStop}>
            {t("proxy.stop")}
          </button>
        )}
      </div>

      {/* Autostart Toggle */}
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

      {/* Toast Message */}
      {message && (
        <div className="toast">
          {message}
        </div>
      )}
    </div>
  );
}
