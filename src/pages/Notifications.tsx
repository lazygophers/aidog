// ─── 应用内通知中心（收件箱）页（N3）────────────────────────
// 侧栏入口 → 历史列表 + 清空。通知完成即结束，无已读未读状态。
// 消费 N1 契约（notificationApi / Notification），只读不改。

import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  notificationApi,
  type Notification,
} from "../services/api";
import { formatDateTime } from "../utils/formatters";
import { Button } from "@/components/ui/button";
import { useReveal, makeRipple } from "../components/shared";

function notifTypeLabel(type: string, t: (k: string, f: string) => string): string {
  return t(`notif.type.${type}`, type);
}

// 单条通知卡：抽离以加 reveal stagger（idx*60），避免 map 内直挂 hook。
function NotificationItem({ item, idx, t }: { item: Notification; idx: number; t: (k: string, f: string, opt?: Record<string, unknown>) => string }) {
  const { ref, shown } = useReveal<HTMLDivElement>(idx * 60);
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
        <div style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4 }}>
          {formatDateTime(item.created_at) || "-"}
        </div>
      </div>
    </div>
  );
}

export function Notifications({ onNavigate }: { onNavigate?: (id: string) => void }) {
  const { t } = useTranslation();
  const [items, setItems] = useState<Notification[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const list = await notificationApi.listInbox();
      setItems(list);
    } catch (e) {
      console.error("load inbox failed", e);
    }
  }, []);

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

      {loading ? (
        <div className="text-secondary" style={{ padding: 20 }}>{t("status.loading", "加载中…")}</div>
      ) : items.length === 0 ? (
        <div className="glass-surface text-secondary" style={{ padding: "40px 20px", textAlign: "center", fontSize: 13 }}>
          {t("notif.inboxEmpty", "暂无通知")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {items.map((item, idx) => (
            <NotificationItem key={item.id} item={item} idx={idx} t={t} />
          ))}
        </div>
      )}
    </div>
  );
}
