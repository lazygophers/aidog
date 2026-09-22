/// 应用内通知中心（收件箱）页（票 I09），对应 `src/pages/Notifications.tsx`。
///
/// 通知完成即结束，**无已读未读状态**：只有「历史列表 + 清空」。
/// 两条命令、一个空态、一个禁用态，所以逻辑与界面同文件，不单开一个 `_logic.dart`。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../utils/formatters.dart';
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'invoke.dart';
import 'platform_card_bits.dart' show MiniBadge;
import 'ui_bits.dart';

/// `types/generated/Notification.ts`。
class NotificationItem {
  const NotificationItem({
    required this.id,
    required this.notifType,
    required this.title,
    required this.body,
    required this.createdAt,
  });

  final int id;
  final String notifType;
  final String title;
  final String body;
  final int createdAt;

  static NotificationItem fromJson(Map<String, Object?> j) => NotificationItem(
    id: (j['id'] as num?)?.toInt() ?? 0,
    notifType: (j['notif_type'] as String?) ?? '',
    title: (j['title'] as String?) ?? '',
    body: (j['body'] as String?) ?? '',
    createdAt: (j['created_at'] as num?)?.toInt() ?? 0,
  );
}

/// `Notifications.tsx:15::notifTypeLabel`。未知类型回落裸值（`t(key, type)` 的第二参）。
String notifTypeLabel(String type, String Function(String) t) {
  final label = t('notif.type.$type');
  return label == 'notif.type.$type' ? type : label;
}

class NotificationsPage extends StatefulWidget {
  const NotificationsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.onNavigate,
  });

  final InvokeFn invoke;

  /// 「通知设置」按钮：切到 `settings/notifications`。不给就不渲染这个按钮（React 同）。
  final void Function(String id)? onNavigate;

  @override
  State<NotificationsPage> createState() => _NotificationsPageState();
}

class _NotificationsPageState extends State<NotificationsPage> {
  List<NotificationItem> _items = const [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    await _refresh();
    if (mounted) setState(() => _loading = false);
  }

  Future<void> _refresh() async {
    try {
      // React 传 `limit`（`notification.ts:14` 的 `{ limit }`），不传即后端默认。
      final raw = await widget.invoke('notification_inbox_list', {
        'limit': null,
      });
      final list = [
        for (final n in (raw as List<Object?>))
          NotificationItem.fromJson((n as Map).cast<String, Object?>()),
      ];
      if (mounted) setState(() => _items = list);
    } catch (_) {
      // React 只 console.error：保留上一次列表，不清空、不弹错。
    }
  }

  Future<void> _clear() async {
    try {
      await widget.invoke('notification_clear');
      await _refresh();
    } catch (_) {
      // 同上。
    }
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        PageHead(
          title: t.t('notif.inboxTitle'),
          trailing: Wrap(
            spacing: AidogSpace.ssm,
            children: [
              if (widget.onNavigate != null)
                SmallButton(
                  label: t.t('notifications.goSettings'),
                  onTap: () => widget.onNavigate!('settings/notifications'),
                ),
              SmallButton(
                label: t.t('notif.clear'),
                // 空列表时「清空」是禁用的（React 的 `disabled={items.length === 0}`）。
                onTap: _items.isEmpty ? null : _clear,
              ),
            ],
          ),
        ),
        if (_loading)
          CenteredNote(text: t.t('status.loading'))
        else if (_items.isEmpty)
          CenteredNote(text: t.t('notif.inboxEmpty'))
        else
          Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              // 入场错峰 index*60 + 悬停抬升（`Notifications.tsx:21,25`）。
              for (final (i, item) in _items.indexed)
                Reveal(
                  delayMs: i * 60,
                  child: HoverLift(
                    child: Padding(
                      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                      child: Container(
                        // 整条左侧 2px accent 竖条（`Notifications.tsx:31`）。
                        decoration: BoxDecoration(
                          border: BorderDirectional(
                            start: BorderSide(color: theme.c.accent, width: 2),
                          ),
                        ),
                        child: Tile(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Row(
                                children: [
                                  Flexible(
                                    child: Text(
                                      // 有标题就「标题 · 类型」，没有就只有类型（React 同）。
                                      item.title.isNotEmpty
                                          ? '${item.title} · ${notifTypeLabel(item.notifType, t.t)}'
                                          : notifTypeLabel(item.notifType, t.t),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: AidogType.body.copyWith(
                                        color: theme.c.fg,
                                      ),
                                    ),
                                  ),
                                  const SizedBox(width: AidogSpace.sxs),
                                  // 类型徽标（`Notifications.tsx:41-51`）。
                                  MiniBadge(
                                    text: notifTypeLabel(item.notifType, t.t),
                                    color: theme.c.accentText,
                                  ),
                                ],
                              ),
                              if (item.body.isNotEmpty)
                                Text(
                                  item.body,
                                  style: AidogType.micro.copyWith(
                                    color: theme.c.fg2,
                                  ),
                                ),
                              Text(
                                // 时间戳缺省时 React 渲染 "-"。
                                ltr(
                                  formatDateTime(item.createdAt).isEmpty
                                      ? '-'
                                      : formatDateTime(item.createdAt),
                                ),
                                style: AidogType.micro.copyWith(
                                  color: theme.c.fg3,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
            ],
          ),
      ],
    );
  }
}
