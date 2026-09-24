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
        ToggleCard(
          key: const ValueKey('notif-master'),
          label: t.t('notif.masterToggle'),
          descriptions: [t.t('notif.masterToggleDesc')],
          value: s.enabled,
          onChanged: (_) => _c.toggleEnabled(),
        ),
        HeaderCard(
          title: t.t('notif.permGuideTitle'),
          descriptions: [t.t('notif.permGuideDesc')],
          trailing: SmallButton(
            key: const ValueKey('open-system-notif'),
            ghost: true,
            fontSize: 12,
            padding: (12, 6),
            label: t.t('notif.permGuideButton'),
            onTap: _c.openSystemNotificationSettings,
          ),
          child: const SizedBox.shrink(),
        ),
        // TTS 独立卡；总开关关掉后压暗（`NotificationSettings.tsx:313`）。
        HeaderCard(
          dimmed: !s.enabled,
          title: t.t('notif.ttsToggle'),
          descriptions: [t.t('notif.ttsToggleDesc')],
          trailing: AidogSwitch(
            key: const ValueKey('notif-tts'),
            value: s.ttsEnabled,
            compact: true,
            onChanged: () => _c.toggleTts(),
          ),
          child: s.ttsEnabled
              ? Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Container(
                    decoration: BoxDecoration(
                      border: Border(
                        top: BorderSide(color: AidogTheme.of(context).c.line),
                      ),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.only(top: AidogSpace.smd),
                      child: InlineRow(
                        label: t.t('notif.ttsBackendLabel'),
                        child: InlineSelect<String>(
                          value: s.ttsBackend,
                          options: kTtsBackends,
                          width: 220,
                          labelOf: (b) => tOr(t, 'notif.ttsBackend.$b', b),
                          onChanged: (v) => _c.setTtsBackend(v!),
                        ),
                      ),
                    ),
                  ),
                )
              : const SizedBox.shrink(),
        ),
        // 通道测试：紧凑单行条（padding 12/20，label 12 w600 与按钮同排，
        // `NotificationSettings.tsx:352-390`）。顺序与 emoji 前缀照 React：
        // 🔊语音 → 🪟弹窗 → 🔔提示音 → 测试。
        _testBar(t, s),
        HeaderCard(
          key: const ValueKey('inbox-retention-on'),
          title: t.t('notif.retentionTitle'),
          descriptions: [t.t('notif.retentionDesc')],
          trailing: AidogSwitch(
            // 关 → 0（不清理）；开 → 回 7 天默认。
            value: s.inboxRetentionDays > 0,
            compact: true,
            onChanged: () =>
                _c.setInboxRetentionDays(s.inboxRetentionDays > 0 ? 0 : 7),
          ),
          child: s.inboxRetentionDays > 0
              ? Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Container(
                    decoration: BoxDecoration(
                      border: Border(
                        top: BorderSide(color: AidogTheme.of(context).c.line),
                      ),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.only(top: AidogSpace.smd),
                      child: InlineRow(
                        label: t.t('notif.retentionDaysLabel'),
                        unit: t.t('notif.retentionDaysUnit'),
                        child: NumberInput(
                          key: const ValueKey('inbox-retention'),
                          value: '${s.inboxRetentionDays}',
                          min: 1,
                          max: 3650,
                          width: 120,
                          onChanged: (v) {
                            // 限 [1,3650]；0 仅由开关切「不清理」。
                            final n = int.tryParse(v) ?? 1;
                            _c.setInboxRetentionDays(n.clamp(1, 3650));
                          },
                        ),
                      ),
                    ),
                  ),
                )
              : const SizedBox.shrink(),
        ),
        ToggleCard(
          key: const ValueKey('default-hooks'),
          label: t.t('notif.defaultHooksTitle'),
          descriptions: [
            _c.hooksDisabled
                ? '${t.t('notif.defaultHooksDesc')} · '
                      '${t.t('notif.defaultHooksDisabledHint')}'
                : t.t('notif.defaultHooksDesc'),
          ],
          value: _c.defaultHooks,
          // 总开关关掉时强制禁用：通知都不发，hook 没意义。
          onChanged: _c.hooksDisabled || _c.defaultHooksBusy
              ? null
              : (_) => _c.toggleDefaultHooks(texts),
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

  /// 通道测试单行条：React `padding: "12px 20px"`、label 12 w600、
  /// 四颗 ghost 按钮 12px（`NotificationSettings.tsx:347-390`）。
  Widget _testBar(I18nController t, NotificationSettings s) {
    final texts = _texts(t);
    return Opacity(
      key: const ValueKey('notif-test-bar-dim'),
      opacity: s.enabled ? 1 : 0.55,
      child: Padding(
        padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
        child: Tile(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
          child: Wrap(
            spacing: AidogSpace.ssm,
            runSpacing: AidogSpace.sxs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Text(
                t.t('notif.testChannels'),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: AidogTheme.of(context).c.fg,
                ),
              ),
              SmallButton(
                ghost: true,
                key: const ValueKey('test-tts'),
                fontSize: 12,
                padding: (10, 4),
                label: '🔊 ${t.t('notif.testTtsLabel')}',
                tooltip: t.t('notif.testTtsTip'),
                onTap: s.enabled ? () => _c.testTts(texts) : null,
              ),
              SmallButton(
                ghost: true,
                key: const ValueKey('test-popup'),
                fontSize: 12,
                padding: (10, 4),
                label: '🪟 ${t.t('notif.testPopupLabel')}',
                tooltip: t.t('notif.testPopupTip'),
                onTap: s.enabled ? () => _c.testPopup(texts) : null,
              ),
              SmallButton(
                ghost: true,
                key: const ValueKey('test-beep'),
                fontSize: 12,
                padding: (10, 4),
                label: '🔔 ${t.t('notif.testBeepLabel')}',
                tooltip: t.t('notif.testBeepTip'),
                onTap: s.enabled ? _c.testBeep : null,
              ),
              SmallButton(
                ghost: true,
                key: const ValueKey('test-notify'),
                fontSize: 12,
                padding: (10, 4),
                label: t.t('notif.test'),
                onTap: s.enabled ? () => _c.testNotify(texts) : null,
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _eventList(I18nController t, NotificationSettings s) {
    final disabled = !s.enabled;
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
      child: Tile(
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              t.t('notif.eventListTitle'),
              style: AidogType.label.copyWith(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: AidogTheme.of(context).c.fg,
              ),
            ),
            Padding(
              padding: const EdgeInsets.only(top: 2),
              child: Text(
                disabled
                    ? '${t.t('notif.eventListDesc')} · '
                          '${t.t('notif.defaultHooksDisabledHint')}'
                    : t.t('notif.eventListDesc'),
                style: AidogType.caption.copyWith(
                  fontSize: 12,
                  color: AidogTheme.of(context).c.fg2,
                ),
              ),
            ),
            const SizedBox(height: AidogSpace.smd),
            // 总开关关掉后整块压暗（`NotificationEventList.tsx:196`）。
            Opacity(
              key: const ValueKey('notif-event-list-dim'),
              opacity: disabled ? 0.5 : 1,
              child: IgnorePointer(
                ignoring: disabled,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    for (final event in orderedHookEvents())
                      _EventRow(
                        key: ValueKey('event-$event'),
                        event: event,
                        setting: effectiveEventSetting(s.perEvent, event),
                        disabled: disabled,
                        // 首次改动即把该事件的当前展示态整份物化进 per_event。
                        onUpdate: (next) =>
                            _c.updateEvent(event, next.toJson()),
                      ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _uvPrompt(I18nController t, NotificationTexts texts) {
    final theme = AidogTheme.of(context);
    // React 是普通 `Dialog`（`NotificationSettings.tsx:483`，maxWidth 420）：
    // 安装中不许关。
    return AidogModal(
      onBarrierTap: _c.uvInstalling ? null : _c.cancelUvPrompt,
      child: ModalCard(
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

/// 一个 hook 事件：盒行（bg-subtle、r8、padding 10/12，`NotificationEventList.tsx:132-141`）
/// 内三行 —— ①启用开关 + 事件名 mono + 三通道开关横排 ②模板 textarea（mono 12、minH 40）
/// ③可用入参 accent chips。
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
    return Container(
      margin: const EdgeInsets.only(bottom: AidogSpace.ssm),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: theme.c.surface2,
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 第一行：启用开关 + 事件名（CC 官方名，不翻译）+ 三通道。
          Wrap(
            spacing: 12,
            runSpacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              AidogSwitch(
                key: ValueKey('event-enabled-$event'),
                value: setting.enabled,
                compact: true,
                onChanged: disabled
                    ? null
                    : () =>
                          onUpdate(setting.copyWith(enabled: !setting.enabled)),
              ),
              SizedBox(
                width: 150,
                child: Text(
                  event,
                  style: AidogType.numMd.copyWith(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: theme.c.fg,
                  ),
                ),
              ),
              _channelSwitch(
                context,
                label: t.t('notif.fieldTts'),
                on: setting.tts,
                disabled: chDisabled,
                key: ValueKey('event-tts-$event'),
                onToggle: () => onUpdate(setting.copyWith(tts: !setting.tts)),
              ),
              _channelSwitch(
                context,
                label: t.t('notif.fieldPopup'),
                on: setting.popup,
                disabled: chDisabled,
                key: ValueKey('event-popup-$event'),
                onToggle: () =>
                    onUpdate(setting.copyWith(popup: !setting.popup)),
              ),
              _channelSwitch(
                context,
                label: t.t('notif.fieldSound'),
                on: setting.sound,
                disabled: chDisabled,
                key: ValueKey('event-sound-$event'),
                onToggle: () =>
                    onUpdate(setting.copyWith(sound: !setting.sound)),
              ),
            ],
          ),
          const SizedBox(height: AidogSpace.ssm),
          // 第二行：模板 textarea，placeholder = 该事件专属默认模板。
          Tooltip(
            message: t.t('notif.fieldTemplate'),
            child: PlainTextField(
              key: ValueKey('event-template-$event'),
              value: setting.template,
              hint: defaultTemplateForEvent(event),
              enabled: setting.enabled,
              mono: true,
              maxLines: null,
              minLines: 2,
              onSubmitted: (v) => onUpdate(setting.copyWith(template: v)),
            ),
          ),
          const SizedBox(height: AidogSpace.ssm),
          // 第三行：该事件专属可用入参提示（每事件不同）。
          Wrap(
            spacing: 6,
            runSpacing: 4,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Text(
                t.t('notif.eventVarsHint'),
                style: AidogType.caption.copyWith(
                  fontSize: 11,
                  color: theme.c.fg2,
                ),
              ),
              for (final v in eventVars(event))
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 6,
                    vertical: 2,
                  ),
                  decoration: BoxDecoration(
                    color: theme.c.accentWash,
                    borderRadius: BorderRadius.circular(AidogRadius.sm),
                  ),
                  child: Text(
                    v,
                    style: AidogType.numSm.copyWith(
                      fontSize: 11,
                      color: theme.c.accentText,
                    ),
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _channelSwitch(
    BuildContext context, {
    Key? key,
    required String label,
    required bool on,
    required bool disabled,
    required VoidCallback onToggle,
  }) {
    final theme = AidogTheme.of(context);
    return Opacity(
      opacity: disabled ? 0.5 : 1,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.only(right: 6),
            child: Text(
              label,
              style: AidogType.caption.copyWith(
                fontSize: 12,
                color: theme.c.fg2,
              ),
            ),
          ),
          AidogSwitch(
            key: key,
            value: on,
            compact: true,
            onChanged: disabled ? null : onToggle,
          ),
        ],
      ),
    );
  }
}
