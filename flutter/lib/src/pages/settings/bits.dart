/// 设置页 13 个子页共用的表单零件（票 I16）。
///
/// 放这里而不是各页一份，理由与 I07 的 `ui_bits.dart` 相同：开关 / 输入框 / 下拉的
/// **禁用条件**是本票的验收项之一，各页各写一个就会各自漂移，测试也要写十三遍。
///
/// 三条约定：
/// 1. 色值一律 `AidogTheme.of(context).c.*`，本文件零硬编码颜色。
/// 2. `onChanged == null` = 禁用态，就是 React 那边 `disabled=` 的直接投影。
/// 3. 受控文本框的 `TextEditingController` 必须活在 State 里（理由见 `groups.dart:727`）。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../shell/app_shell.dart';
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../ui_bits.dart';

/// 取文案，缺 key 时回落到 [fallback]。
///
/// Dart 侧 `t()` 缺 key 返回 key 本身（`translations.dart:75`），而 React 的
/// `t(key, fallback)` 有第二参。schema 里的 `label` 就是那个 fallback ——
/// 没有这个函数，465 个字段里没配 i18n 的就会显示裸 key。
String tOr(I18nController t, String key, String fallback) {
  final s = t.t(key);
  return s == key ? fallback : s;
}

/// 一块设置分区：标题 + 说明 + 若干行。
class SettingsCard extends StatelessWidget {
  const SettingsCard({
    super.key,
    this.title,
    this.meta,
    this.description,
    required this.children,
  });

  final String? title;
  final String? meta;
  final String? description;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.smd),
      child: Tile(
        title: title,
        meta: meta,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            if (description != null && description!.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                child: Text(
                  description!,
                  style: AidogType.micro.copyWith(color: theme.c.fg3),
                ),
              ),
            ...children,
          ],
        ),
      ),
    );
  }
}

/// 开关行。`onChanged == null` → 点不动且变灰。
class SwitchRow extends StatelessWidget {
  const SwitchRow({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    this.description,
  });

  final String label;
  final String? description;
  final bool value;
  final ValueChanged<bool>? onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final fg = onChanged == null ? theme.c.fg3 : theme.c.fg;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AidogSpace.sxs),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(label, style: AidogType.label.copyWith(color: fg)),
                if (description != null && description!.isNotEmpty)
                  Text(
                    description!,
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
              ],
            ),
          ),
          const SizedBox(width: AidogSpace.ssm),
          Switch(
            value: value,
            onChanged: onChanged,
            activeThumbColor: theme.c.accent,
            inactiveTrackColor: theme.c.surface2,
            inactiveThumbColor: theme.c.fg3,
          ),
        ],
      ),
    );
  }
}

/// 受控文本框行。[onSubmitted] 用于 onBlur / 回车才提交的场景。
class TextRow extends StatefulWidget {
  const TextRow({
    super.key,
    required this.label,
    required this.value,
    this.onChanged,
    this.onSubmitted,
    this.description,
    this.hint,
    this.maxLines = 1,
    this.obscure = false,
  });

  final String label;
  final String? description;
  final String? hint;
  final String value;
  final ValueChanged<String>? onChanged;

  /// 失焦或回车时调用（React 的 `onBlur`）。
  final ValueChanged<String>? onSubmitted;
  final int maxLines;
  final bool obscure;

  @override
  State<TextRow> createState() => _TextRowState();
}

class _TextRowState extends State<TextRow> {
  late final TextEditingController _ctrl = TextEditingController(
    text: widget.value,
  );
  final FocusNode _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      if (!_focus.hasFocus) widget.onSubmitted?.call(_ctrl.text);
    });
  }

  @override
  void didUpdateWidget(TextRow old) {
    super.didUpdateWidget(old);
    // 外部改了值才同步回输入框，免得打断用户正在选的那一段。
    if (widget.value != _ctrl.text && !_focus.hasFocus) {
      _ctrl.value = TextEditingValue(
        text: widget.value,
        selection: TextSelection.collapsed(offset: widget.value.length),
      );
    }
  }

  @override
  void dispose() {
    _focus.dispose();
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final enabled = widget.onChanged != null || widget.onSubmitted != null;
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          TileMeta(widget.label),
          if (widget.description != null && widget.description!.isNotEmpty)
            Text(
              widget.description!,
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          TextField(
            controller: _ctrl,
            focusNode: _focus,
            enabled: enabled,
            maxLines: widget.obscure ? 1 : widget.maxLines,
            obscureText: widget.obscure,
            style: AidogType.micro.copyWith(
              color: enabled ? theme.c.fg : theme.c.fg3,
            ),
            decoration: InputDecoration(
              isDense: true,
              hintText: widget.hint,
              hintStyle: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
            onChanged: widget.onChanged,
            onSubmitted: widget.onSubmitted,
          ),
        ],
      ),
    );
  }
}

/// 数字输入行。`parse` 由调用方给（各页的取整 / 钳位规则不同，不在这里假定）。
class NumberRow extends StatelessWidget {
  const NumberRow({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    this.description,
    this.parse,
  });

  final String label;
  final String? description;
  final int value;
  final ValueChanged<int>? onChanged;
  final int Function(String raw)? parse;

  @override
  Widget build(BuildContext context) => TextRow(
    label: label,
    description: description,
    value: '$value',
    onSubmitted: onChanged == null
        ? null
        : (v) => onChanged!((parse ?? _default)(v)),
  );

  static int _default(String raw) => int.tryParse(raw.trim()) ?? 0;
}

/// 单选行：一排 [SmallButton]，选中的高亮。选项少（≤8）时比下拉更好点。
class ChoiceRow extends StatelessWidget {
  const ChoiceRow({
    super.key,
    required this.label,
    required this.options,
    required this.value,
    required this.onChanged,
    this.labelOf,
    this.description,
  });

  final String label;
  final String? description;
  final List<String> options;
  final String value;
  final ValueChanged<String>? onChanged;
  final String Function(String option)? labelOf;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          TileMeta(label),
          if (description != null && description!.isNotEmpty)
            Text(
              description!,
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          const SizedBox(height: AidogSpace.sxs),
          Wrap(
            spacing: AidogSpace.sxs,
            runSpacing: AidogSpace.sxs,
            children: [
              for (final o in options)
                SmallButton(
                  label: labelOf?.call(o) ?? o,
                  active: o == value,
                  onTap: onChanged == null ? null : () => onChanged!(o),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

/// 选项很多（语言 36 项、env 的 select）时用真下拉，不铺成一排。
class SelectRow extends StatelessWidget {
  const SelectRow({
    super.key,
    required this.label,
    required this.options,
    required this.value,
    required this.onChanged,
    this.labelOf,
    this.description,
  });

  final String label;
  final String? description;
  final List<String> options;
  final String value;
  final ValueChanged<String?>? onChanged;
  final String Function(String option)? labelOf;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    // 当前值不在选项里（用户手填的自定义值）也要能显示，否则 Dropdown 会断言失败。
    final items = <String>[
      if (value.isNotEmpty && !options.contains(value)) value,
      ...options,
    ];
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          TileMeta(label),
          if (description != null && description!.isNotEmpty)
            Text(
              description!,
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          DropdownButton<String>(
            value: value.isEmpty ? null : value,
            // Material 默认在下拉底下画一条横线，与本项目的描边风格冲突。
            underline: const SizedBox.shrink(),
            isDense: true,
            isExpanded: true,
            dropdownColor: theme.c.surface2,
            style: AidogType.micro.copyWith(color: theme.c.fg),
            hint: Text(
              '—',
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
            onChanged: onChanged,
            items: [
              for (final o in items)
                DropdownMenuItem<String>(
                  value: o,
                  child: Text(labelOf?.call(o) ?? o),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

/// 一句话的只读信息行（指纹、路径、上次备份时间…）。
class InfoRow extends StatelessWidget {
  const InfoRow({super.key, required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: AidogType.micro.copyWith(color: theme.c.fg3)),
          const SizedBox(width: AidogSpace.ssm),
          Expanded(
            child: Text(
              value,
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
          ),
        ],
      ),
    );
  }
}

/// 常驻错误条（写失败之类，不自动消失）。与 [ToastBar] 的区别是它不带计时器。
class ErrorNote extends StatelessWidget {
  const ErrorNote({super.key, required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.ssm),
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.symmetric(
          horizontal: AidogSpace.smd,
          vertical: AidogSpace.ssm,
        ),
        decoration: BoxDecoration(
          color: theme.c.surface2,
          border: Border.all(color: theme.c.bad),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: Text(
          text,
          style: AidogType.micro.copyWith(color: theme.c.bad),
        ),
      ),
    );
  }
}

/// 未保存确认卡，三个出口与 `Settings.tsx` 的弹窗一一对应：
/// 保存并离开 / 放弃更改 / 取消（留在本页，guard 仍挂着）。
class UnsavedChangesCard extends StatelessWidget {
  const UnsavedChangesCard({
    super.key,
    required this.onSave,
    required this.onDiscard,
    required this.onCancel,
    this.busy = false,
  });

  final VoidCallback onSave;
  final VoidCallback onDiscard;
  final VoidCallback onCancel;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: t.t('settings.unsavedTitle'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              t.t('settings.unsavedBody'),
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  key: const ValueKey('unsaved-cancel'),
                  label: t.t('action.cancel'),
                  onTap: busy ? null : onCancel,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  key: const ValueKey('unsaved-discard'),
                  label: t.t('settings.discardChanges'),
                  danger: true,
                  onTap: busy ? null : onDiscard,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  key: const ValueKey('unsaved-save'),
                  label: t.t('settings.saveAndLeave'),
                  onTap: busy ? null : onSave,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// 3 秒后自动消失的提示条。计时器挂在 widget 上，控制器不碰时间。
class AutoToast extends StatefulWidget {
  const AutoToast({
    super.key,
    required this.text,
    required this.onDone,
    this.ok = true,
  });

  final String text;
  final bool ok;
  final VoidCallback onDone;

  @override
  State<AutoToast> createState() => _AutoToastState();
}

class _AutoToastState extends State<AutoToast> {
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _arm();
  }

  @override
  void didUpdateWidget(AutoToast old) {
    super.didUpdateWidget(old);
    if (old.text != widget.text) _arm();
  }

  void _arm() {
    _timer?.cancel();
    _timer = Timer(const Duration(seconds: 3), () {
      if (mounted) widget.onDone();
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      ToastBar(text: widget.text, ok: widget.ok);
}

/// 无标签的受控文本框（编辑器内部的行用，如权限规则 pattern、hooks 命令）。
/// 与 [TextRow] 同一套同步规则：外部值变了才写回 controller，不打断正在编辑的光标。
class PlainTextField extends StatefulWidget {
  const PlainTextField({
    super.key,
    required this.value,
    this.onChanged,
    this.onSubmitted,
    this.hint,
    this.maxLines = 1,
    this.enabled = true,
  });

  final String value;
  final String? hint;
  final ValueChanged<String>? onChanged;
  final ValueChanged<String>? onSubmitted;
  final int maxLines;
  final bool enabled;

  @override
  State<PlainTextField> createState() => _PlainTextFieldState();
}

class _PlainTextFieldState extends State<PlainTextField> {
  late final TextEditingController _ctrl = TextEditingController(
    text: widget.value,
  );
  final FocusNode _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      if (!_focus.hasFocus) widget.onSubmitted?.call(_ctrl.text);
    });
  }

  @override
  void didUpdateWidget(PlainTextField old) {
    super.didUpdateWidget(old);
    if (widget.value != _ctrl.text && !_focus.hasFocus) {
      _ctrl.value = TextEditingValue(
        text: widget.value,
        selection: TextSelection.collapsed(offset: widget.value.length),
      );
    }
  }

  @override
  void dispose() {
    _focus.dispose();
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return TextField(
      controller: _ctrl,
      focusNode: _focus,
      enabled: widget.enabled,
      maxLines: widget.maxLines,
      style: AidogType.micro.copyWith(
        color: widget.enabled ? theme.c.fg : theme.c.fg3,
      ),
      decoration: InputDecoration(
        isDense: true,
        hintText: widget.hint,
        hintStyle: AidogType.micro.copyWith(color: theme.c.fg3),
      ),
      onChanged: widget.onChanged,
      onSubmitted: widget.onSubmitted,
    );
  }
}

/// 子页的统一外壳：标题 + 右上角动作 + 内容。
class SettingsPageBody extends StatelessWidget {
  const SettingsPageBody({
    super.key,
    required this.title,
    this.subtitle,
    this.trailing,
    required this.children,
  });

  final String title;
  final String? subtitle;
  final Widget? trailing;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      PageHead(title: title, subtitle: subtitle, trailing: trailing),
      ...children,
    ],
  );
}
