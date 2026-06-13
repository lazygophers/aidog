// ─── 应用内通知中心（收件箱）页（N3）────────────────────────
// 侧栏入口 → 历史列表 + 未读计数 + 标记已读（单条/全部）+ 清空。
// listen NOTIF_INBOX_UPDATED 实时刷新。消费 N1 契约（notificationApi / Notification），只读不改。

import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import {
  notificationApi,
  NOTIF_INBOX_UPDATED,
  type Notification,
} from "../services/api";

function notifTypeLabel(type: string, t: (k: string, f: string) => string): string {
  return t(`notif.type.${type}`, type);
}

export function Notifications() {
  const { t } = useTranslation();
  const [items, setItems] = useState<Notification[]>([]);
  const [unread, setUnread] = useState(0);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [list, count] = await Promise.all([
        notificationApi.listInbox(),
        notificationApi.unreadCount(),
      ]);
      setItems(list);
      setUnread(count);
    } catch (e) {
      console.error("load inbox failed", e);
    }
  }, []);

  useEffect(() => {
    (async () => {
      await refresh();
      setLoading(false);
    })();
    const unlistenPromise = listen(NOTIF_INBOX_UPDATED, () => { refresh(); });
    return () => { unlistenPromise.then((un) => un()).catch((e) => console.error(e)); };
  }, [refresh]);

  const handleMarkRead = async (id?: number) => {
    try {
      await notificationApi.markRead(id);
      await refresh();
    } catch (e) {
      console.error("mark read failed", e);
    }
  };

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
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <h2 style={{ fontSize: 18, fontWeight: 700, margin: 0 }}>{t("notif.inboxTitle", "通知中心")}</h2>
          {unread > 0 && (
            <span
              style={{
                fontSize: 11,
                fontWeight: 700,
                padding: "2px 8px",
                borderRadius: 999,
                background: "var(--accent)",
                color: "#fff",
              }}
            >
              {unread} {t("notif.unread", "未读")}
            </span>
          )}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12 }}
            disabled={unread === 0}
            onClick={() => handleMarkRead()}
          >
            {t("notif.markAllRead", "全部标记已读")}
          </button>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12 }}
            disabled={items.length === 0}
            onClick={handleClear}
          >
            {t("notif.clear", "清空")}
          </button>
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
          {items.map((item) => (
            <div
              key={item.id}
              className="glass-surface"
              style={{
                padding: "12px 16px",
                display: "flex",
                gap: 12,
                alignItems: "flex-start",
                opacity: item.read ? 0.6 : 1,
                borderInlineStart: item.read ? "2px solid transparent" : "2px solid var(--accent)",
              }}
            >
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 2 }}>
                  <span style={{ fontSize: 13, fontWeight: 600 }}>{item.title || notifTypeLabel(item.notif_type, t)}</span>
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
                  {new Date(item.created_at).toLocaleString()}
                </div>
              </div>
              {!item.read && (
                <button
                  className="btn btn-ghost"
                  style={{ fontSize: 11, padding: "3px 8px", whiteSpace: "nowrap" }}
                  onClick={() => handleMarkRead(item.id)}
                >
                  {t("notif.markRead", "已读")}
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
