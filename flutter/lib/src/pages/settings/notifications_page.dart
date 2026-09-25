/// 系统通知页（`settings/notifications`）的 widget 层 —— 对齐
/// `src/components/settings/NotificationSettings.tsx` + `NotificationEventList.tsx`。
///
/// 防抖落盘、失败回滚、uv 询问框的状态机全在 [NotificationsController]
/// （票 I08 已测），这里只画界面。
library;

import 'dart:async';

import 'package:flutter/foundation.dart' show defaultTargetPlatform;
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

/// 卡与卡之间的间距：React 三个设置 tab 的容器全是 `gap: 20`
/// （`src/pages/AppSettings.tsx:71`、`NotificationSettings.tsx:270`）。
const double _cardGap = 20;

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
        // React 加载态继承正文字号（`NotificationSettings.tsx:263`）。
        children: [CenteredNote(text: t.t('status.loading'), fontSize: 13)],
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
          bottomGap: _cardGap,
          hoverLift: true,
          value: s.enabled,
          onChanged: (_) => _c.toggleEnabled(),
        ),
        // 🔴 仅 macOS 渲染这张引导卡：React 的 `IS_MACOS` 门
        //（`NotificationSettings.tsx:54,289`），Windows / Linux 通知一般默认可用。
        if (defaultTargetPlatform == TargetPlatform.macOS)
          HeaderCard(
            title: t.t('notif.permGuideTitle'),
            descriptions: [t.t('notif.permGuideDesc')],
            bottomGap: _cardGap,
            hoverLift: true,
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
          bottomGap: _cardGap,
          hoverLift: true,
          title: t.t('notif.ttsToggle'),
          descriptions: [t.t('notif.ttsToggleDesc')],
          trailing: AidogSwitch(
            key: const ValueKey('notif-tts'),
            value: s.ttsEnabled,
            compact: true,
            onChanged: () => _c.toggleTts(),
          ),
          child: s.ttsEnabled
              // 展开区：线上 12（卡的 `gap: 12`）/ 线下 8（`paddingTop: 8`），
              // `NotificationSettings.tsx:328`。
              ? Padding(
                  padding: const EdgeInsets.only(top: 12),
                  child: Container(
                    decoration: BoxDecoration(
                      border: Border(
                        top: BorderSide(color: AidogTheme.of(context).c.line),
                      ),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.only(top: 8),
                      child: InlineRow(
                        label: t.t('notif.ttsBackendLabel'),
                        child: InlineSelect<String>(
                          value: s.ttsBackend,
                          options: kTtsBackends,
                          width: 220,
                          // React `SelectTrigger` 描边盒 + `padding: "4px 8px"`
                          //（`NotificationSettings.tsx:338`）。
                          bordered: true,
                          boxPadding: const EdgeInsets.symmetric(
                            horizontal: 8,
                            vertical: 4,
                          ),
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
          bottomGap: _cardGap,
          hoverLift: true,
          trailing: AidogSwitch(
            // 关 → 0（不清理）；开 → 回 7 天默认。
            value: s.inboxRetentionDays > 0,
            compact: true,
            onChanged: () =>
                _c.setInboxRetentionDays(s.inboxRetentionDays > 0 ? 0 : 7),
          ),
          child: s.inboxRetentionDays > 0
              // 线上 12 / 线下 8（`NotificationSettings.tsx:400,417`）。
              ? Padding(
                  padding: const EdgeInsets.only(top: 12),
                  child: Container(
                    decoration: BoxDecoration(
                      border: Border(
                        top: BorderSide(color: AidogTheme.of(context).c.line),
                      ),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.only(top: 8),
                      child: InlineRow(
                        label: t.t('notif.retentionDaysLabel'),
                        unit: t.t('notif.retentionDaysUnit'),
                        // 本页这个单位 React 是 secondary，不是 tertiary
                        //（`NotificationSettings.tsx:434`）。
                        unitColor: AidogTheme.of(context).c.fg2,
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
          bottomGap: _cardGap,
          hoverLift: true,
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
          // 页容器 gap 20（`NotificationSettings.tsx:270`）。
          padding: EdgeInsets.only(bottom: _cardGap),
          child: PiUnsupportedNote(
            reasonKey: 'pi.unsupportedHooks',
            reasonFallback: 'pi 没有配置式 hooks，事件处理只能写在 extension 的 TypeScript 里，aidog 无法代为注入。',
          ),
        ),
        _eventList(t, s),
        if (_c.uvPrompt != null) _uvPrompt(t, texts),
        // 错误 / 提示都是页面流里的 `.toast` 方条（`NotificationSettings.tsx:473-478`），
        // 不是浮在窗口顶部的胶囊。
        if (_c.error.isNotEmpty)
          InlineNote(
            text: _c.error,
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            fontSize: 12,
          ),
        if (_c.message.isNotEmpty)
          AutoToast(
            text: _c.message,
            inline: true,
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
        padding: const EdgeInsets.only(bottom: _cardGap),
        child: HoverLift(
          child: Tile(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
          child: Wrap(
            // 条内元素间距横竖同为 8（`NotificationSettings.tsx:352`）。
            spacing: 8,
            runSpacing: 8,
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
      ),
    );
  }

  Widget _eventList(I18nController t, NotificationSettings s) {
    final disabled = !s.enabled;
    return Padding(
      padding: const EdgeInsets.only(bottom: _cardGap),
      // 列表卡 React 也挂 `hover-lift`（`NotificationEventList.tsx:185`）。
      child: HoverLift(
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
        // `DialogContent` 自带 ✕（安装中不给关，与遮罩同口径）。
        onClose: _c.uvInstalling ? null : _c.cancelUvPrompt,
        // `padding: "20px 24px"`、标题 15 w600（`NotificationSettings.tsx:483-488`）。
        padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 20),
        titleStyle: AidogType.title.copyWith(
          fontSize: 15,
          fontWeight: FontWeight.w600,
        ),
        title: t.t('notif.uvModalTitle'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              // 说明 13、行高 1.5（`NotificationSettings.tsx:489`）。
              t.t('notif.uvModalDesc'),
              style: AidogType.caption.copyWith(
                fontSize: 13,
                height: 1.5,
                color: theme.c.fg2,
              ),
            ),
            // `DialogContent` 的 `gap-4` = 16（`src/components/ui/dialog.tsx:41`）。
            const SizedBox(height: 16),
            // 三颗都是 12 / 6-12，彼此 gap 8；前两颗 ghost、第三颗实心
            //（`NotificationSettings.tsx:490-514`）。
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  key: const ValueKey('uv-cancel'),
                  ghost: true,
                  fontSize: 12,
                  padding: (12, 6),
                  label: t.t('notif.uvModalCancel'),
                  onTap: _c.uvInstalling ? null : _c.cancelUvPrompt,
                ),
                const SizedBox(width: 8),
                SmallButton(
                  key: const ValueKey('uv-python3'),
                  ghost: true,
                  fontSize: 12,
                  padding: (12, 6),
                  label: t.t('notif.uvModalUsePython'),
                  onTap: _c.uvInstalling ? null : _c.chooseUsePython3,
                ),
                const SizedBox(width: 8),
                SmallButton(
                  key: const ValueKey('uv-install'),
                  filled: true,
                  fontSize: 12,
                  padding: (12, 6),
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
      // 行间 gap 8（`NotificationEventList.tsx:196`）。
      margin: const EdgeInsets.only(bottom: 8),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        // React 是 `var(--bg-subtle, rgba(127,127,127,0.06))`，而 `--bg-subtle`
        // 全库没有定义 —— 实际生效的是回退值「中性灰 6%」
        //（`NotificationEventList.tsx:142`）。
        color: theme.c.fg.withValues(alpha: 0.06),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 第一行：启用开关 + 事件名（CC 官方名，不翻译）+ 三通道。
          Wrap(
            // `gap: 12` 同时管列与行（`NotificationEventList.tsx:203`）。
            spacing: 12,
            runSpacing: 12,
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
              ConstrainedBox(
                // React 是 `minWidth: 150`，长事件名照样撑开
                //（`NotificationEventList.tsx:211`）；原来的固定宽会把它截掉。
                constraints: const BoxConstraints(minWidth: 150),
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
          // 行容器 `gap: 8`（`NotificationEventList.tsx:139`）。
          const SizedBox(height: 8),
          // 第二行：模板 textarea，placeholder = 该事件专属默认模板。
          Tooltip(
            message: t.t('notif.fieldTemplate'),
            child: PlainTextField(
              key: ValueKey('event-template-$event'),
              value: setting.template,
              hint: defaultTemplateForEvent(event),
              enabled: setting.enabled,
              mono: true,
              // React textarea 是 12（`NotificationEventList.tsx:245`）。
              fontSize: 12,
              maxLines: null,
              minLines: 2,
              onSubmitted: (v) => onUpdate(setting.copyWith(template: v)),
            ),
          ),
          const SizedBox(height: 8),
          // 第三行：该事件专属可用入参提示（每事件不同）。
          Wrap(
            // `gap: 6` 横竖同值（`NotificationEventList.tsx:254`）。
            spacing: 6,
            runSpacing: 6,
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
                    // `var(--radius-sm)` = 6（`NotificationEventList.tsx:262`）。
                    borderRadius: BorderRadius.circular(6),
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
    // 只有开关自己压暗（shadcn `disabled:opacity-50`），「语音 / 弹窗 / 提示音」
    // 三个标签不变灰（`NotificationEventList.tsx:213-239`）。
    return Row(
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
        Opacity(
          opacity: disabled ? 0.5 : 1,
          child: AidogSwitch(
            key: key,
            value: on,
            compact: true,
            onChanged: disabled ? null : onToggle,
          ),
        ),
      ],
    );
  }
}
