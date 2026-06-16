# PRD: 通知去已读未读状态(保留历史)

## 背景
通知系统含 read 列 + 未读 badge + markRead commands。用户要求去已读未读, 通知完成即结束(但仍保留历史可查)。

## 用户决策
保留历史(收件箱页 + notification 表), 仅删 read 列 + 未读 badge + mark_read command + count_unread。

## 范围 (单交付, main worktree 内直接写)

### 后端
- `db.rs`:
  - 017 DDL 块: 去 `read` 列 + 删 `idx_notif_read` index
  - 新 018 migration 块: `DROP INDEX idx_notif_read; ALTER TABLE notification DROP COLUMN read;`
  - `insert_notification`: INSERT 去 read
  - `list_notifications`: SELECT 去 read
  - 删 `count_unread_notifications` / `mark_notification_read`
  - 保留 `clear_notifications`
- `models.rs`: `Notification` struct 去 `read` 字段
- `notification.rs`: 删 `NOTIF_INBOX_UPDATED` 常量 + unread emit 逻辑
- `lib.rs`: 删 `notification_inbox_unread` / `notification_inbox_mark_read` commands + invoke_handler 注册

### 前端
- `api.ts`: 删 `notificationApi.unreadCount` / `markRead`; `Notification` type 去 `read`
- `App.tsx`: 删 unread state + refreshUnread + listen(NOTIF_INBOX_UPDATED) + badge 设置
- `pages/Notifications.tsx`: 删 unread state + 未读计数 + "全部标记已读"按钮 + 单条 markRead 按钮 + item.read 样式区分; 保留历史列表 + 清空
- 8 locale: 删 `notif.unread` / `notif.markRead` / `notif.markAllRead` key

## 验证
- `cargo test gateway::db` 绿 (加 migration 测? DROP COLUMN)
- `cargo clippy` 无 warning (删完函数无 dead_code)
- `yarn build` exit 0
- check-i18n 零缺失
- 实跑: 通知落库 → 收件箱页展示历史无已读未读区分 + 无 badge

## 不做
- 不删 notification 表 / 收件箱页(保留历史)
- 不删 NotifForm.InboxOnly(仍需 inbox 通道落库)
- 不删 clear_notifications(清空历史仍需)
