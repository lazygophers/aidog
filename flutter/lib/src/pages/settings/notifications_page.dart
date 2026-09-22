/// 系统通知页（`settings/notifications`）的 widget 层 —— 对齐
/// `src/components/settings/NotificationSettings.tsx` + `NotificationEventList.tsx`。
///
/// 防抖落盘、失败回滚、uv 询问框的状态机全在 [NotificationsController]
/// （票 I08 已测），这里只画界面。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../../platform.dart' as native;
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'notification_events.dart';
import 'notifications_logic.dart';

class NotificationsSettingsPage extends StatefulWidget {
  const NotificationsSettingsPage({
    super.key,
    this.invoke = kernelInvoke,
    this.onEnabledChanged,
    this.openUrlFn,
  });

  final InvokeFn invoke;

  /// 总开关变化上抛给外壳（关掉通知时侧栏藏掉「通知」项）。
  final void Function(bool enabled)? onEnabledChanged;
  final Future<void> Function(String url)? openUrlFn;

  @override
  State<NotificationsSettingsPage> createState() =>
      _NotificationsSettingsPageState();
}

class _NotificationsSettingsPageState extends State<NotificationsSettingsPage> {
  late final NotificationsController _c;

  @override
  void initState() {
    super.initState();
    _c = NotificationsController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
      onEnabledChanged: widget.onEnabledChanged,
      openUrlFn: widget.openUrlFn ?? native.openUrl,
    );
    unawaited(_c.load());
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  NotificationTexts _texts(I18nController t) => NotificationTexts(
    testSent: t.t('notif.testSent'),
    testTtsContent: t.t('notif.testTtsContent'),
    testPopupTitle: t.t('notif.testPopupTitle'),
    testPopupBody: t.t('notif.testPopupBody'),
    uvInstalled: t.t('notif.uvInstalled'),
    uvInstallFailed: t.t('notif.uvInstallFailed'),
    defaultHooksOn: t.t('notif.defaultHooksOn'),
    defaultHooksOff: t.t('notif.defaultHooksOff'),
  );

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final texts = _texts(t);
    if (_c.loading) {
      return SettingsPageBody(
        title: t.t('appSettings.notificationsTab'),
        children: [CenteredNote(text: t.t('status.loading'))],
      );
    }
    final s = _c.settings;
    return SettingsPageBody(
      title: t.t('appSettings.notificationsTab'),
      children: [
        SettingsCard(
          children: [
            SwitchRow(
              key: const ValueKey('notif-master'),
              label: t.t('notif.masterToggle'),
              description: t.t('notif.masterToggleDesc'),
              value: s.enabled,
              onChanged: (_) => _c.toggleEnabled(),
            ),
            SwitchRow(
              key: const ValueKey('notif-tts'),
              label: t.t('notif.ttsToggle'),
              description: t.t('notif.ttsToggleDesc'),
              value: s.ttsEnabled,
              onChanged: (_) => _c.toggleTts(),
            ),
            if (s.ttsEnabled)
              ChoiceRow(
                label: t.t('notif.ttsBackendLabel'),
                options: kTtsBackends,
                value: s.ttsBackend,
                labelOf: (b) => tOr(t, 'notif.ttsBackend.$b', b),
                onChanged: _c.setTtsBackend,
              ),
          ],
        ),
        SettingsCard(
          title: t.t('notif.permGuideTitle'),
          description: t.t('notif.permGuideDesc'),
          children: [
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: SmallButton(
                key: const ValueKey('open-system-notif'),
                label: t.t('notif.permGuideButton'),
                onTap: _c.openSystemNotificationSettings,
              ),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('notif.testChannels'),
          children: [
            Wrap(
              spacing: AidogSpace.ssm,
              runSpacing: AidogSpace.sxs,
              children: [
                SmallButton(
                  key: const ValueKey('test-notify'),
                  label: t.t('notif.test'),
                  onTap: () => _c.testNotify(texts),
                ),
                SmallButton(
                  key: const ValueKey('test-tts'),
                  label: t.t('notif.testTtsLabel'),
                  onTap: () => _c.testTts(texts),
                ),
                SmallButton(
                  key: const ValueKey('test-popup'),
                  label: t.t('notif.testPopupLabel'),
                  onTap: () => _c.testPopup(texts),
                ),
                SmallButton(
                  key: const ValueKey('test-beep'),
                  label: t.t('notif.testBeepLabel'),
                  onTap: _c.testBeep,
                ),
              ],
            ),
          ],
        ),
        SettingsCard(
          title: t.t('notif.retentionTitle'),
          description: t.t('notif.retentionDesc'),
          children: [
            NumberRow(
              key: const ValueKey('inbox-retention'),
              label:
                  '${t.t('notif.retentionDaysLabel')} '
                  '(${t.t('notif.retentionDaysUnit')})',
              value: s.inboxRetentionDays,
              onChanged: (v) => _c.setInboxRetentionDays(v < 0 ? 0 : v),
            ),
          ],
        ),
        SettingsCard(
          title: t.t('notif.defaultHooksTitle'),
          description: _c.hooksDisabled
              ? '${t.t('notif.defaultHooksDesc')} · '
                    '${t.t('notif.defaultHooksDisabledHint')}'
              : t.t('notif.defaultHooksDesc'),
          children: [
            SwitchRow(
              key: const ValueKey('default-hooks'),
              label: t.t('notif.defaultHooksTitle'),
              value: _c.defaultHooks,
              // 总开关关掉时强制禁用：通知都不发，hook 没意义。
              onChanged: _c.hooksDisabled || _c.defaultHooksBusy
                  ? null
                  : (_) => _c.toggleDefaultHooks(texts),
            ),
          ],
        ),
        const Padding(
          padding: EdgeInsets.only(bottom: AidogSpace.smd),
          child: PiUnsupportedNote(
            reasonKey: 'pi.unsupportedHooks',
            reasonFallback: 'pi 没有配置式 hooks，事件处理只能写在 extension 的 TypeScript 里，aidog 无法代为注入。',
          ),
        ),
        _eventList(t, s),
        if (_c.uvPrompt != null) _uvPrompt(t, texts),
        if (_c.error.isNotEmpty) ErrorNote(text: _c.error),
        if (_c.message.isNotEmpty)
          AutoToast(
            text: _c.message,
            onDone: () => setState(() => _c.message = ''),
          ),
      ],
    );
  }

  Widget _eventList(I18nController t, NotificationSettings s) {
    final disabled = !s.enabled;
    return SettingsCard(
      title: t.t('notif.eventListTitle'),
      description: disabled
          ? '${t.t('notif.eventListDesc')} · '
                '${t.t('notif.defaultHooksDisabledHint')}'
          : t.t('notif.eventListDesc'),
      children: [
        for (final event in orderedHookEvents())
          _EventRow(
            key: ValueKey('event-$event'),
            event: event,
            setting: effectiveEventSetting(s.perEvent, event),
            disabled: disabled,
            // 首次改动即把该事件的当前展示态整份物化进 per_event。
            onUpdate: (next) => _c.updateEvent(event, next.toJson()),
          ),
      ],
    );
  }

  Widget _uvPrompt(I18nController t, NotificationTexts texts) {
    final theme = AidogTheme.of(context);
    // React 是普通 `Dialog`（`NotificationSettings.tsx:483`，maxWidth 420）：
    // 安装中不许关。
    return AidogModal(
      onBarrierTap: _c.uvInstalling ? null : _c.cancelUvPrompt,
      child: Tile(
        title: t.t('notif.uvModalTitle'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              t.t('notif.uvModalDesc'),
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  key: const ValueKey('uv-cancel'),
                  label: t.t('notif.uvModalCancel'),
                  onTap: _c.uvInstalling ? null : _c.cancelUvPrompt,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  key: const ValueKey('uv-python3'),
                  label: t.t('notif.uvModalUsePython'),
                  onTap: _c.uvInstalling ? null : _c.chooseUsePython3,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  key: const ValueKey('uv-install'),
                  label: _c.uvInstalling
                      ? t.t('notif.uvModalInstalling')
                      : t.t('notif.uvModalInstall'),
                  onTap: _c.uvInstalling
                      ? null
                      : () => _c.chooseInstallUv(texts),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// 一个 hook 事件：启用 / 语音 / 弹窗 / 提示音 四个开关 + 模板 + 可用入参。
///
/// 三个通道开关在**事件本身没启用时**一律禁用（React 的
/// `disabled={disabled || !es.enabled}`）。
class _EventRow extends StatelessWidget {
  const _EventRow({
    super.key,
    required this.event,
    required this.setting,
    required this.disabled,
    required this.onUpdate,
  });

  final String event;
  final EventSetting setting;
  final bool disabled;
  final ValueChanged<EventSetting> onUpdate;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final chDisabled = disabled || !setting.enabled;
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          SwitchRow(
            // 事件名英文原样（Claude Code 官方名，不翻译）。
            label: event,
            value: setting.enabled,
            onChanged: disabled
                ? null
                : (v) => onUpdate(setting.copyWith(enabled: v)),
          ),
          Wrap(
            spacing: AidogSpace.ssm,
            children: [
              _toggleChip(
                context,
                label: t.t('notif.fieldTts'),
                on: setting.tts,
                onTap: chDisabled
                    ? null
                    : () => onUpdate(setting.copyWith(tts: !setting.tts)),
              ),
              _toggleChip(
                context,
                label: t.t('notif.fieldPopup'),
                on: setting.popup,
                onTap: chDisabled
                    ? null
                    : () => onUpdate(setting.copyWith(popup: !setting.popup)),
              ),
              _toggleChip(
                context,
                label: t.t('notif.fieldSound'),
                on: setting.sound,
                onTap: chDisabled
                    ? null
                    : () => onUpdate(setting.copyWith(sound: !setting.sound)),
              ),
            ],
          ),
          TextRow(
            label: t.t('notif.fieldTemplate'),
            hint: defaultTemplateForEvent(event),
            value: setting.template,
            maxLines: 2,
            onSubmitted: setting.enabled
                ? (v) => onUpdate(setting.copyWith(template: v))
                : null,
          ),
          Text(
            '${t.t('notif.eventVarsHint')}: ${eventVars(event).join(' ')}',
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
        ],
      ),
    );
  }

  Widget _toggleChip(
    BuildContext context, {
    required String label,
    required bool on,
    required VoidCallback? onTap,
  }) => SmallButton(label: label, active: on, onTap: onTap);
}
