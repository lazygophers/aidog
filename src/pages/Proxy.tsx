import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { proxyApi } from "../services/api";

export function Proxy() {
  const { t } = useTranslation();
  const [running, setRunning] = useState(false);
  const [port, setPort] = useState(8080);
  const [message, setMessage] = useState("");

  const checkStatus = async () => {
    try {
      const s = await proxyApi.status();
      setRunning(s);
    } catch {
      setRunning(false);
    }
  };

  useEffect(() => { checkStatus(); }, []);

  const handleStart = async () => {
    try {
      const msg = await proxyApi.start(port);
      setRunning(true);
      setMessage(msg);
    } catch (e: any) {
      setMessage(e.toString());
    }
  };

  const handleStop = async () => {
    try {
      await proxyApi.stop();
      setRunning(false);
      setMessage(t("proxy.stopped"));
    } catch (e: any) {
      setMessage(e.toString());
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, maxWidth: 600, width: "100%" }}>
      <h2 style={{ fontSize: 20, fontWeight: 600 }}>{t("page.proxy")}</h2>

      {/* Status card */}
      <div className="glass" style={{ padding: 24, textAlign: "center" }}>
        <div style={{
          fontSize: 48, marginBottom: 8,
          filter: running ? "none" : "grayscale(1) opacity(0.4)",
        }}>
          {running ? "🟢" : "⚫"}
        </div>
        <div style={{ fontSize: 16, fontWeight: 600, marginBottom: 4 }}>
          {running ? t("proxy.running") : t("proxy.stopped")}
        </div>
        {running && (
          <div className="text-secondary" style={{ fontSize: 13 }}>
            {t("proxy.listening")} localhost:{port}
          </div>
        )}
      </div>

      {/* Controls */}
      <div className="glass-surface" style={{ padding: 20, display: "flex", gap: 12, alignItems: "center" }}>
        <label style={{ fontSize: 13, whiteSpace: "nowrap" }}>{t("proxy.port")}</label>
        <input className="input" type="number" value={port}
          onChange={(e) => setPort(Number(e.target.value))}
          disabled={running}
          style={{ width: 100 }} />
        <div style={{ flex: 1 }} />
        {!running ? (
          <button className="btn btn-primary" onClick={handleStart}>{t("proxy.start")}</button>
        ) : (
          <button className="btn" onClick={handleStop} style={{ borderColor: "red", color: "red" }}>
            {t("proxy.stop")}
          </button>
        )}
      </div>

      {/* Config hint */}
      <div className="glass-surface" style={{ padding: 16, fontSize: 13 }}>
        <div style={{ fontWeight: 600, marginBottom: 8 }}>{t("proxy.configHint")}</div>
        <code style={{
          display: "block", padding: 12, borderRadius: "var(--radius-sm)",
          background: "var(--bg-base)", fontSize: 12, lineHeight: 1.6,
          wordBreak: "break-all",
        }}>
          ANTHROPIC_BASE_URL=http://localhost:{port}/claude/v1/messages
        </code>
        <div className="text-secondary" style={{ marginTop: 8, fontSize: 12 }}>
          {t("proxy.configDesc")}
        </div>
      </div>

      {message && (
        <div style={{ padding: 12, fontSize: 13, borderRadius: "var(--radius-sm)",
          background: "var(--accent-subtle)", color: "var(--accent)" }}>
          {message}
        </div>
      )}
    </div>
  );
}
