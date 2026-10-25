// ─── 应用内通知中心（收件箱）页（N3）────────────────────────
// 侧栏入口 → 历史列表 + 清空。通知完成即结束，无已读未读状态。
// 消费 N1 契约（notificationApi / Notification），只读不改。

import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  notificationApi,
  type Notification,
  type NotificationDelivery,
} from "../services/api";
import { formatDateTime } from "../utils/formatters";
import { Button } from "@/components/ui/button";
import { useReveal, makeRipple } from "../components/shared";

function notifTypeLabel(type: string, t: (k: string, f: string) => string): string {
  return t(`notif.type.${type}`, type);
}

// 单条通知卡：抽离以加 reveal stagger（idx*60），避免 map 内直挂 hook。
// 票 04：来源徽标（hook/proxy/api）+ 来源跳转（proxy→平台页、hook→日志页）+ 投递记录展开。
function NotificationItem({ item, idx, t, onNavigate }: { item: Notification; idx: number; t: (k: string, f: string, opt?: Record<string, unknown>) => string; onNavigate?: (id: string) => void }) {
  const { ref, shown } = useReveal<HTMLDivElement>(idx * 60);
  const [deliveries, setDeliveries] = useState<NotificationDelivery[] | null>(null);
  const loadDeliveries = async () => {
    if (deliveries) { setDeliveries(null); return; }
    try {
      const rows = await notificationApi.listDeliveries(item.action_key || undefined, 50);
      setDeliveries(rows.filter(d => !item.action_key || d.action_key === item.action_key));
    } catch (e) {
      console.error("load deliveries failed", e);
      setDeliveries([]);
    }
  };
  return (
    <div
      ref={ref}
      className={`glass-surface hover-lift reveal${shown ? " in" : ""}`}
      style={{
        padding: "12px 16px",
        display: "flex",
        gap: 12,
        alignItems: "flex-start",
        borderInlineStart: "2px solid var(--accent)",
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 2 }}>
          <span style={{ fontSize: 13, fontWeight: 600 }}>
            {item.title
              ? `${item.title} · ${notifTypeLabel(item.notif_type, t)}`
              : notifTypeLabel(item.notif_type, t)}
          </span>
          <span
            style={{
              fontSize: 10,
              padding: "1px 6px",
              borderRadius: "var(--radius-sm)",
              background: "var(--accent-subtle)",
              color: "var(--accent)",
            }}
          >
            {notifTypeLabel(item.notif_type, t)}
          </span>
        </div>
        {item.body && (
          <div style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
            {item.body}
          </div>
        )}
        <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4, display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          <span>{formatDateTime(item.created_at) || "-"}</span>
          {item.source && (
            <span id={`notif-src-${item.id}`} style={{ fontSize: 10, padding: "0 5px", borderRadius: "var(--radius-sm)", background: "var(--bg-glass)" }}>
              {t(`notif.source.${item.source}`, item.source)}
            </span>
          )}
          {item.severity && item.severity !== "info" && (
            <span style={{ fontSize: 10, padding: "0 5px", borderRadius: "var(--radius-sm)", background: item.severity === "error" ? "var(--status-error-bg, rgba(248,113,113,.15))" : "var(--status-warn-bg, rgba(251,191,36,.15))" }}>
              {t(`notif.severity.${item.severity}`, item.severity)}
            </span>
          )}
          {item.source === "proxy" && onNavigate && (
            <Button variant="ghost" style={{ fontSize: 10, height: "auto", padding: "1px 7px" }} onClick={() => onNavigate("platforms")}>
              {t("notif.jump.platforms", "平台页")}
            </Button>
          )}
          {(item.source === "hook" || item.source === "proxy") && onNavigate && (
            <Button variant="ghost" style={{ fontSize: 10, height: "auto", padding: "1px 7px" }} onClick={() => onNavigate("logs")}>
              {t("notif.jump.logs", "日志")}
            </Button>
          )}
          {item.source === "hook" && item.vars?.cwd && (
            <span style={{ fontFamily: "var(--font-mono, monospace)" }}>{item.vars.cwd}</span>
          )}
          <Button variant="ghost" style={{ fontSize: 10, height: "auto", padding: "1px 7px" }} onClick={loadDeliveries}>
            {deliveries ? t("notif.inbox.hideDeliveries", "收起投递") : t("notif.inbox.showDeliveries", "投递记录")}
          </Button>
        </div>
        {deliveries && (
          <div id={`notif-deliveries-${item.id}`} data-count={deliveries.length} style={{ marginTop: 6, borderTop: "1px dashed var(--border)", paddingTop: 6 }}>
            {deliveries.length === 0
              ? <span className="text-tertiary" style={{ fontSize: 11 }}>{t("notif.inbox.noDeliveries", "无外部投递记录（本地渠道不留痕）")}</span>
              : deliveries.map(d => (
                <div key={d.id} style={{ fontSize: 11, display: "flex", gap: 8 }}>
                  <span style={{ minWidth: 64 }}>{t(`notif.channel.${d.channel}`, d.channel)}</span>
                  <span data-ok={d.status === "success" ? "yes" : "no"} style={{ minWidth: 44, color: d.status === "success" ? "var(--status-ok, #4ade80)" : "var(--status-error, #f87171)" }}>
                    {d.status === "success" ? "✓" : "✗"} {d.duration_ms}ms
                  </span>
                  {d.error && <span className="text-tertiary" style={{ wordBreak: "break-all" }}>{d.error}</span>}
                </div>
              ))}
          </div>
        )}
      </div>
    </div>
  );
}

export function Notifications({ onNavigate }: { onNavigate?: (id: string) => void }) {
  const { t } = useTranslation();
  const [items, setItems] = useState<Notification[]>([]);
  const [loading, setLoading] = useState(true);
  // 票 04 筛选：源 / 类型 / 级别 / 时间窗（24h / 7d / 全部）。
  const [fSource, setFSource] = useState("");
  const [fSeverity, setFSeverity] = useState("");
  const [fTime, setFTime] = useState("");

  const refresh = useCallback(async () => {
    try {
      const sinceMs = fTime === "24h" ? Date.now() - 24 * 3600_000
        : fTime === "7d" ? Date.now() - 7 * 24 * 3600_000
        : undefined;
      const list = await notificationApi.listInbox(200, {
        source: fSource || undefined,
        severity: fSeverity || undefined,
        sinceMs,
      });
      setItems(list);
    } catch (e) {
      console.error("load inbox failed", e);
    }
  }, [fSource, fSeverity, fTime]);

  useEffect(() => { refresh(); }, [refresh]);

  useEffect(() => {
    (async () => {
      await refresh();
      setLoading(false);
    })();
  }, [refresh]);

  const handleClear = async () => {
    try {
      await notificationApi.clearInbox();
      await refresh();
    } catch (e) {
      console.error("clear inbox failed", e);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h2 style={{ fontSize: 18, fontWeight: 700, margin: 0 }}>{t("notif.inboxTitle", "通知中心")}</h2>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          {onNavigate && (
            <Button
              variant="ghost"
              className="ripple"
              style={{ fontSize: 12, height: "auto", padding: "4px 10px" }}
              onClick={(e) => { makeRipple(e); onNavigate("settings/notifications"); }}
            >
              {t("notifications.goSettings", "通知设置")}
            </Button>
          )}
          <Button
            variant="ghost"
            className="ripple"
            style={{ fontSize: 12, height: "auto", padding: "4px 10px" }}
            disabled={items.length === 0}
            onClick={(e) => { makeRipple(e); handleClear(); }}
          >
            {t("notif.clear", "清空")}
          </Button>
        </div>
      </div>

      {/* 筛选（票 04）：源 / 级别 / 时间 */}
      <div id="notif-filter-bar" style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <label style={{ fontSize: 11, display: "flex", gap: 4, alignItems: "center" }}>
          <span className="text-secondary">{t("notif.filter.source", "来源")}</span>
          <select className="input" style={{ fontSize: 12, padding: "2px 6px" }} value={fSource} onChange={e => setFSource(e.target.value)}>
            <option value="">{t("notif.filter.all", "全部")}</option>
            <option value="hook">hook</option>
            <option value="proxy">proxy</option>
            <option value="api">api</option>
          </select>
        </label>
        <label style={{ fontSize: 11, display: "flex", gap: 4, alignItems: "center" }}>
          <span className="text-secondary">{t("notif.filter.severity", "级别")}</span>
          <select className="input" style={{ fontSize: 12, padding: "2px 6px" }} value={fSeverity} onChange={e => setFSeverity(e.target.value)}>
            <option value="">{t("notif.filter.all", "全部")}</option>
            <option value="info">info</option>
            <option value="warn">warn</option>
            <option value="error">error</option>
          </select>
        </label>
        <label style={{ fontSize: 11, display: "flex", gap: 4, alignItems: "center" }}>
          <span className="text-secondary">{t("notif.filter.time", "时间")}</span>
          <select className="input" style={{ fontSize: 12, padding: "2px 6px" }} value={fTime} onChange={e => setFTime(e.target.value)}>
            <option value="">{t("notif.filter.all", "全部")}</option>
            <option value="24h">24h</option>
            <option value="7d">7d</option>
          </select>
        </label>
      </div>

      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading", "加载中…")}</div>
      ) : items.length === 0 ? (
        <div className="glass-surface text-secondary" style={{ padding: "40px 20px", textAlign: "center", fontSize: 13 }}>
          {t("notif.inboxEmpty", "暂无通知")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {items.map((item, idx) => (
            <NotificationItem key={item.id} item={item} idx={idx} t={t} onNavigate={onNavigate} />
          ))}
        </div>
      )}
    </div>
  );
}
