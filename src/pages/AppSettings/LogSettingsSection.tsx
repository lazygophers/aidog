import { useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { proxyLogApi } from "../../services/api/proxy";
import type { SystemSettings } from "./useSystemSettings";

/**
 * Log recording + Application Logging 两个日志相关 section（原 L562-678 + L757-818）。
 */
export function LogSettingsSection({ s }: { s: SystemSettings }) {
  const { t } = useTranslation();
  const {
    logEnabled, logUserReq, logUpstreamReq,
    userReqRetention, upstreamReqRetention, logRetention,
    setLogUserReq, setLogUpstreamReq,
    setUserReqRetention, setUpstreamReqRetention, setLogRetention,
    handleLogEnabledChange, updateLogSettings,
    logFileEnabled, logLevel, logRetHours, handleLogSettingsChange,
  } = s;

  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ text: string; type: "success" | "error" } | null>(null);

  function flashMessage(text: string, type: "success" | "error" = "success") {
    setMessage({ text, type });
    setTimeout(() => setMessage(null), 3000);
  }

  async function handleCleanupExpired() {
    if (busy) return;
    setBusy(true);
    try {
      await proxyLogApi.cleanupExpired();
      flashMessage(t("logs.cleanupExpiredDone", "已清理过期日志"), "success");
    } catch (e) {
      console.error("[LogSettings] cleanupExpired failed", e);
      flashMessage(String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleClearConfirm() {
    setBusy(true);
    try {
      await proxyLogApi.clear();
      setShowClearConfirm(false);
      flashMessage(t("logs.clearDone", "已清空"), "success");
    } catch (e) {
      flashMessage(String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
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

        {/* Cleanup actions — 独立于 logEnabled：关闭记录后仍需可清已存日志 */}
        <div style={{ display: "flex", flexDirection: "column", gap: 4, paddingTop: 8, borderTop: "1px solid var(--border)" }}>
          <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
            <button
              className="btn"
              onClick={handleCleanupExpired}
              disabled={busy || logRetention === 0}
              title={logRetention === 0 ? t("logs.cleanupDisabledHint", "永久保留模式，无过期日志可清理") : undefined}
              style={{ fontSize: 12, padding: "4px 12px", opacity: busy ? 0.6 : 1 }}
            >
              {busy ? t("logs.cleaning", "清理中...") : t("logs.cleanupExpired", "清理过期")}
            </button>
            <button
              className="btn btn-danger"
              onClick={() => setShowClearConfirm(true)}
              disabled={busy}
              style={{ fontSize: 12, padding: "4px 12px", opacity: busy ? 0.6 : 1 }}
            >
              {busy ? t("logs.cleaning", "清理中...") : t("logs.clear", "清除全部")}
            </button>
          </div>
          {message && (
            <div
              className="toast"
              style={{
                fontSize: 12,
                marginTop: 4,
                color: message.type === "success"
                  ? "var(--color-success, #22c55e)"
                  : "var(--color-error, #ef4444)",
              }}
            >
              {message.text}
            </div>
          )}
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

      {/* 清空确认 modal — 必须 portal document.body（祖先 transform/backdrop-filter 让 fixed 退化） */}
      {showClearConfirm && createPortal(
        <div
          style={{
            position: "fixed", inset: 0, display: "flex", alignItems: "center", justifyContent: "center",
            background: "rgba(0, 0, 0, 0.4)", zIndex: 1000,
          }}
          onClick={() => !busy && setShowClearConfirm(false)}
        >
          <div
            className="glass-surface"
            style={{
              padding: 20, maxWidth: 380, borderRadius: "var(--radius-lg)",
              display: "flex", flexDirection: "column", gap: 16,
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{ fontSize: 13, fontWeight: 600 }}>
              {t("logs.clearConfirmTitle", "清空全部日志")}
            </div>
            <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("logs.clearConfirm", "确认清除所有日志？此操作不可撤销。")}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button
                className="btn"
                onClick={() => setShowClearConfirm(false)}
                disabled={busy}
                style={{ padding: "6px 14px", fontSize: 12 }}
              >
                {t("logs.cancel", "取消")}
              </button>
              <button
                className="btn btn-danger"
                onClick={handleClearConfirm}
                disabled={busy}
                style={{
                  padding: "6px 14px", fontSize: 12,
                  opacity: busy ? 0.6 : 1,
                }}
              >
                {busy ? t("logs.cleaning", "清理中...") : t("logs.clear", "清除全部")}
              </button>
            </div>
          </div>
        </div>,
        document.body
      )}
    </>
  );
}
