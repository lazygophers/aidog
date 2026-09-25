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
    this.dimmed = false,
    this.icon,
    this.emphasized = false,
    required this.children,
  });

  final String? title;
  final String? meta;
  final String? description;

  /// 节标题行首图标（React 各节 20px 的 `SectionIcon`，`Settings.tsx:527-531`）。
  final IconData? icon;

  /// 大节标题：20px w600（editors 的 F.title）。缺省 false = Tile 的 13.5。
  final bool emphasized;

  /// 总开关关掉后压暗这张卡（React 四处 `opacity: 0.55 / 0.5`：
  /// `SchedulingSettings.tsx:142`、`NotificationEventList.tsx:196`、
  /// `NotificationSettings.tsx:313,352`、`MiddlewareRules.tsx:1246`）。
  /// 只是**弱化**，不禁用 —— 禁用与否由各行自己的 `onChanged` 决定。
  final bool dimmed;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Opacity(
      opacity: dimmed ? 0.55 : 1,
      child: Padding(
        // React 设置页 section 卡间距 20（AppSettings.tsx:71 的 gap: 20、
        // editors/tokens.ts 的 S.sectionGap）。
        padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
        child: Tile(
          // 大节标题不借 Tile 自带的 13.5 标题，自己在正文顶上画 20px 那行。
          title: emphasized ? null : title,
          meta: meta,
          // React 设置分区卡 padding 28（editors/tokens.ts:15 的 S.pad）。
          padding: const EdgeInsets.all(28),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              if (emphasized && title != null)
                Padding(
                  padding: const EdgeInsets.only(bottom: 22),
                  child: Row(
                    children: [
                      if (icon != null) ...[
                        Icon(icon, size: 20, color: theme.c.fg),
                        const SizedBox(width: 8),
                      ],
                      Text(
                        title!,
                        style: AidogType.title.copyWith(
                          fontSize: 20,
                          letterSpacing: -0.2,
                          color: theme.c.fg,
                        ),
                      ),
                    ],
                  ),
                ),
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
    this.hint,
    this.labelIcon,
    this.trailing,
  });

  final String label;

  /// 标签行首的图标（同 [TextRow.labelIcon]）。
  final IconData? labelIcon;
  final String? description;

  /// 「落点」：这个开关到底改哪个文件的哪个键
  /// （`CodingToolsSettings.tsx` 的 `ToggleCard hint`，等宽小字）。
  /// 不写就不占位。
  final String? hint;
  final bool value;
  final ValueChanged<bool>? onChanged;

  /// 控件右侧的附加按钮（环境变量编辑器的「移除」× 就挂这里）。
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final fg = onChanged == null ? theme.c.fg3 : theme.c.fg;
    // 根因 A：React 开关行标题是 13px w600 正文级（`StartupSection.tsx:26-28`
    // 等全部开关卡），描述 12px text-secondary。原先 label 13.5 w400 + micro 11，
    // 整套低一档。
    final titleStyle = AidogType.label.copyWith(
      fontSize: 13,
      fontWeight: FontWeight.w600,
      color: fg,
    );
    final descStyle = AidogType.caption.copyWith(
      fontSize: 12,
      color: onChanged == null ? theme.c.fg3 : theme.c.fg2,
    );
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
                if (labelIcon == null)
                  HighlightedText(label, style: titleStyle)
                else
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(labelIcon, size: 13, color: theme.c.fg3),
                      const SizedBox(width: 5),
                      Flexible(child: HighlightedText(label, style: titleStyle)),
                    ],
                  ),
                if (description != null && description!.isNotEmpty) ...[
                  const SizedBox(height: 2),
                  Text(description!, style: descStyle),
                ],
                if (hint != null && hint!.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Text(
                      // 路径与键名是标识串，RTL 下不该被重排。
                      ltr(hint!),
                      style: AidogType.numSm.copyWith(color: theme.c.fg3),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(width: AidogSpace.ssm),
          // React 把 × 放在开关**左边**（`EnvEditor.tsx:48`），这里照做。
          ?trailing,
          // 36×20 的 shadcn Switch（ui/switch.tsx:14-23），不再用 Material 原件
          // （≈52×32，尺寸与形态都对不上）。禁用态压暗对齐 React 的
          // `disabled:opacity-50`。
          Opacity(
            opacity: onChanged == null ? 0.5 : 1,
            child: AidogSwitch(
              value: value,
              compact: true,
              onChanged: onChanged == null ? null : () => onChanged!(!value),
            ),
          ),
        ],
      ),
    );
  }
}

/// 根因 B：「一开关一卡」。React 设置页的每个开关各自一张 glass 卡
/// （padding 16/20、标题 13 w600 + 描述 12，开关居右，`StartupSection.tsx:19-38`、
/// `MitmConfig.tsx:259-270`、`SchedulingSettings.tsx:100-111` 等）；
/// Flutter 原先多行并进一张 SettingsCard，整页卡片节奏两边完全不同。
///
/// [descriptions] 是多行说明（bindLan 那种两句），一行一个元素，行距 2。
class ToggleCard extends StatelessWidget {
  const ToggleCard({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    this.descriptions,
    this.hint,
  });

  final String label;

  /// 说明行。单句传一个元素即可；null/空 = 无说明区。
  final List<String>? descriptions;

  /// 等宽小字「落点」（coding 四开关那种 hint，`CodingToolsSettings.tsx` ToggleCard）。
  final String? hint;
  final bool value;
  final ValueChanged<bool>? onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final descs = descriptions ?? const <String>[];
    final fg = onChanged == null ? theme.c.fg3 : theme.c.fg;
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
      child: Tile(
        // React 的开关卡是 `padding: "16px 20px"`（竖 16 / 横 20），
        // 不是 editors 分区卡的 28。
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    label,
                    style: AidogType.label.copyWith(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: fg,
                    ),
                  ),
                  for (final d in descs.where((e) => e.isNotEmpty))
                    Padding(
                      padding: const EdgeInsets.only(top: 2),
                      child: Text(
                        d,
                        style: AidogType.caption.copyWith(
                          fontSize: 12,
                          color: onChanged == null ? theme.c.fg3 : theme.c.fg2,
                        ),
                      ),
                    ),
                  if (hint != null && hint!.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.only(top: 4),
                      child: Text(
                        ltr(hint!),
                        style: AidogType.numSm.copyWith(color: theme.c.fg3),
                      ),
                    ),
                ],
              ),
            ),
            const SizedBox(width: AidogSpace.smd),
            Opacity(
              opacity: onChanged == null ? 0.5 : 1,
              child: AidogSwitch(
                value: value,
                compact: true,
                onChanged: onChanged == null ? null : () => onChanged!(!value),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// 带页头的设置卡：标题 13 w600 + 说明 12 在左、[trailing]（通常是开关）在右，
/// 正文 [child] 在下。React 的「总开关卡 + 展开区」都是这个形态
/// （`LogSettingsSection.tsx:104-112`、`UpstreamProxySection`、`NotificationSettings.tsx:311`）。
class HeaderCard extends StatelessWidget {
  const HeaderCard({
    super.key,
    required this.title,
    this.descriptions,
    this.trailing,
    this.dimmed = false,
    required this.child,
  });

  final String title;
  final List<String>? descriptions;
  final Widget? trailing;
  final bool dimmed;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final descs = descriptions ?? const <String>[];
    return Opacity(
      opacity: dimmed ? 0.55 : 1,
      child: Padding(
        padding: const EdgeInsets.only(bottom: AidogSpace.sxl),
        child: Tile(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            mainAxisSize: MainAxisSize.min,
            children: [
              Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          title,
                          style: AidogType.label.copyWith(
                            fontSize: 13,
                            fontWeight: FontWeight.w600,
                            color: theme.c.fg,
                          ),
                        ),
                        for (final d in descs.where((e) => e.isNotEmpty))
                          Padding(
                            padding: const EdgeInsets.only(top: 2),
                            child: Text(
                              d,
                              style: AidogType.caption.copyWith(
                                fontSize: 12,
                                color: theme.c.fg2,
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                  if (trailing != null) ...[
                    const SizedBox(width: AidogSpace.smd),
                    trailing!,
                  ],
                ],
              ),
              child,
            ],
          ),
        ),
      ),
    );
  }
}

/// 根因 C：横排小表单行。React 的 label（12px nowrap）+ 小输入框
/// （h28、宽 70-120）+ 单位在同一行（`ProxyStatusSection.tsx:130-220`、
/// `LogSettingsSection.tsx:144-206`、`SystemMiscSection.tsx:38-65`）；
/// Flutter 原先一律「标签在上 + 控件全宽在下」。
///
/// [child] 是控件本体（自带宽度）；[unit] 是右侧的「秒 / 天」这类单位字。
class InlineRow extends StatelessWidget {
  const InlineRow({
    super.key,
    required this.label,
    required this.child,
    this.labelWidth,
    this.unit,
    this.suffix,
  });

  final String label;

  /// label 列的最小宽度（React 的 `minWidth: 120`）；null = 按文字收缩。
  final double? labelWidth;

  final Widget child;

  /// 单位字（「秒」「天」），12px tertiary。
  final String? unit;

  /// 单位之后还要摆的东西（保留期的单位下拉就挂这里）。
  final Widget? suffix;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final label = Text(
      this.label,
      style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg2),
    );
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        if (labelWidth == null)
          Padding(
            padding: const EdgeInsets.only(right: AidogSpace.ssm),
            child: label,
          )
        else
          ConstrainedBox(
            constraints: BoxConstraints(minWidth: labelWidth!),
            child: Padding(
              padding: const EdgeInsets.only(right: AidogSpace.ssm),
              child: label,
            ),
          ),
        child,
        if (unit != null) ...[
          const SizedBox(width: AidogSpace.ssm),
          Text(
            unit!,
            style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg3),
          ),
        ],
        if (suffix != null) ...[const SizedBox(width: AidogSpace.ssm), suffix!],
      ],
    );
  }
}

/// 横排行里的小下拉（React `UnitSelect` / 协议 Select：宽 80-100、h28、12px）。
/// [SelectRow] 是纵向全宽形态，横排场景塞不进去。
class InlineSelect<T> extends StatelessWidget {
  const InlineSelect({
    super.key,
    required this.value,
    required this.options,
    required this.onChanged,
    this.labelOf,
    this.width = 80,
  });

  final T value;
  final List<T> options;
  final ValueChanged<T?>? onChanged;
  final String Function(T option)? labelOf;

  /// null = 不限宽（字段行右列那种占满剩余空间的下拉）。
  final double? width;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final dropdown = DropdownButton<T>(
        value: value,
        underline: const SizedBox.shrink(),
        isDense: true,
        isExpanded: true,
        dropdownColor: theme.c.surface2,
        style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg),
        onChanged: onChanged,
        items: [
          for (final o in options)
            DropdownMenuItem<T>(
              value: o,
              child: Text(labelOf?.call(o) ?? '$o'),
            ),
        ],
      );
    return width == null ? dropdown : SizedBox(width: width, child: dropdown);
  }
}

/// 图标按钮（React 的 ghost icon button：14px 图标、无描边、悬浮解释挂 title）。
/// 中间件规则行的编辑 / 删除、MITM 白名单的 ✕ 都是它。
class IconGhostButton extends StatelessWidget {
  const IconGhostButton({
    super.key,
    required this.icon,
    this.onTap,
    this.tooltip,
    this.danger = false,
    this.size = 14,
  });

  final IconData icon;
  final VoidCallback? onTap;
  final String? tooltip;
  final bool danger;
  final double size;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final color = onTap == null
        ? theme.c.fg3
        : danger
        ? theme.c.bad
        : theme.c.fg2;
    final btn = InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(AidogRadius.sm),
      child: Padding(
        padding: const EdgeInsets.all(4),
        child: Icon(icon, size: size, color: color),
      ),
    );
    final tip = tooltip;
    if (tip == null || tip.isEmpty) return btn;
    return Tooltip(message: tip, child: btn);
  }
}

/// 分段单选（React `primitives.tsx:243-285` 的 Segmented：外框 r-sm、
/// 段间 borderLeft 分隔、选中段 accentWash 底）。导入冲突行三段决策用它。
class SegmentedRow<T> extends StatelessWidget {
  const SegmentedRow({
    super.key,
    required this.options,
    required this.value,
    required this.onChanged,
    this.labelOf,
    this.fontSize = 12,
    this.segmentKeyOf,
  });

  final List<T> options;
  final T value;
  final ValueChanged<T>? onChanged;
  final String Function(T option)? labelOf;
  final double fontSize;

  /// 每段的 key（测试按段拍冲突决策用）。
  final Key Function(T option)? segmentKeyOf;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Container(
      decoration: BoxDecoration(
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.sm),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final (i, o) in options.indexed)
            Container(
              decoration: BoxDecoration(
                border: i == 0
                    ? null
                    : Border(left: BorderSide(color: theme.c.line)),
                borderRadius: BorderRadius.horizontal(
                  left: i == 0
                      ? Radius.circular(AidogRadius.sm)
                      : Radius.zero,
                  right: i == options.length - 1
                      ? Radius.circular(AidogRadius.sm)
                      : Radius.zero,
                ),
                color: o == value ? theme.c.accentWash : null,
              ),
              child: InkWell(
                key: segmentKeyOf?.call(o),
                onTap: onChanged == null ? null : () => onChanged!(o),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 5,
                  ),
                  child: Text(
                    labelOf?.call(o) ?? '$o',
                    style: AidogType.caption.copyWith(
                      fontSize: fontSize,
                      color: o == value ? theme.c.accentText : theme.c.fg2,
                      fontWeight: o == value ? FontWeight.w600 : FontWeight.w400,
                    ),
                  ),
                ),
              ),
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
    this.labelIcon,
    required this.value,
    this.onChanged,
    this.onSubmitted,
    this.onEnter,
    this.description,
    this.hint,
    this.maxLines = 1,
    this.minLines,
    this.mono = false,
    this.obscure = false,
    this.trailing,
  });

  /// 输入框右侧的附加按钮（环境变量编辑器的「移除」× 就挂这里）。
  final Widget? trailing;

  /// 标签行首的图标。长表单里纯文字标题难扫读，React 各字段行都带一个
  ///（`HooksSectionInline.tsx:227` 等的 `FieldRow icon=`）。
  final IconData? labelIcon;

  final String label;
  final String? description;
  final String? hint;
  final String value;
  final ValueChanged<String>? onChanged;

  /// 失焦或回车时调用（React 的 `onBlur`）。
  final ValueChanged<String>? onSubmitted;

  /// **只有按回车**才调用（React 的 `onKeyDown` + `e.key === "Enter"`）。
  /// 与 [onSubmitted] 分开是因为后者失焦也会触发 —— 「点到别处就把这条规则加进去」
  /// 不是 React 的行为，也不是用户想要的。
  final ValueChanged<String>? onEnter;
  final int maxLines;

  /// 起始行数。不给就是「从一行起、随内容长到 [maxLines]」；
  /// 粘贴整份 JSON 那种框要直接给出足够高度，从一行起等于逼人边粘边滚。
  final int? minLines;

  /// 等宽字。JSON / 命令这类内容缩进对不齐就看不出层级。
  final bool mono;
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
    final enabled =
        widget.onChanged != null ||
        widget.onSubmitted != null ||
        widget.onEnter != null;
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          FieldLabel(widget.label, icon: widget.labelIcon),
          if (widget.description != null && widget.description!.isNotEmpty)
            Text(
              widget.description!,
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(
                child: TextField(
                  controller: _ctrl,
                  focusNode: _focus,
                  enabled: enabled,
                  maxLines: widget.obscure ? 1 : widget.maxLines,
                  minLines: widget.maxLines == 1
                      ? null
                      : (widget.minLines ?? 1),
                  obscureText: widget.obscure,
                  style: (widget.mono ? AidogType.numSm : AidogType.label)
                      .copyWith(color: enabled ? theme.c.fg : theme.c.fg3),
                  decoration: InputDecoration(
                    isDense: true,
                    hintText: widget.hint,
                    hintStyle: AidogType.label.copyWith(color: theme.c.fg3),
                  ),
                  onChanged: widget.onChanged,
                  onSubmitted: (v) {
                    widget.onSubmitted?.call(v);
                    widget.onEnter?.call(v);
                  },
                ),
              ),
              if (widget.trailing != null) ...[
                const SizedBox(width: AidogSpace.sxs),
                widget.trailing!,
              ],
            ],
          ),
        ],
      ),
    );
  }
}

/// 数字输入行：标签 + 说明 + [NumberInput]。
///
/// `parse` 由调用方给（各页的取整规则不同，不在这里假定）；夹取交给
/// [NumberInput]，调用方不必再写 `max(0, ...)`。
class NumberRow extends StatelessWidget {
  const NumberRow({
    super.key,
    required this.label,
    this.labelIcon,
    required this.value,
    required this.onChanged,
    this.description,
    this.parse,
    this.min = 0,
    this.max,
    this.step = 1,
  });

  final String label;

  /// 标签行首的图标（同 [TextRow.labelIcon]）。
  final IconData? labelIcon;
  final String? description;
  final int value;
  final ValueChanged<int>? onChanged;
  final int Function(String raw)? parse;

  /// 见 [NumberInput.min]：默认 0，`0` 本身仍是合法值。
  final num? min;
  final num? max;
  final num step;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        FieldLabel(label, icon: labelIcon),
        if (description != null && description!.isNotEmpty)
          Text(
            description!,
            style: AidogType.micro.copyWith(
              color: AidogTheme.of(context).c.fg3,
            ),
          ),
        NumberInput(
          value: '$value',
          min: min,
          max: max,
          step: step,
          onChanged: onChanged == null
              ? null
              : (v) => onChanged!((parse ?? _default)(v)),
        ),
      ],
    ),
  );

  static int _default(String raw) => int.tryParse(raw.trim()) ?? 0;
}

/// 多选下拉：收起时显示已选项（空 = [emptyLabel]），展开是可滚的勾选清单。
///
/// 对齐 React 的 `MultiSelect`（`MiddlewareRules.tsx:117-157`：触发器显示
/// 「A、B」，展开 `maxHeight: 280` 可滚，逐项 checkbox）。选项**多到平铺会撑爆**
/// 的维度才用它 —— 平台 / 分组几十条一铺，下面的维度就被挤到屏幕外了；
/// 选项 ≤8 的仍用 [ChoiceRow]，平铺少点一次更好用。
class MultiSelectRow extends StatelessWidget {
  const MultiSelectRow({
    super.key,
    required this.label,
    required this.options,
    required this.selected,
    required this.onToggle,
    required this.emptyLabel,
    this.itemKeyPrefix,
  });

  final String label;

  /// 候选。`value` 是 `Object`：平台维度是 `int` id，分组维度是 `String` key，
  /// 两者的类型要原样带到后端（见 `AppliesToEditor` 顶部那段注释）。
  final List<({Object value, String label})> options;
  final List<Object> selected;
  final ValueChanged<Object> onToggle;

  /// 一条都没选时触发器上显示什么（通常是「全部」）。
  final String emptyLabel;

  /// 清单每行的 key 前缀（测试按 `<prefix>-<value>` 点具体某一项）。
  final String? itemKeyPrefix;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final chosen = [
      for (final o in options)
        if (selected.contains(o.value)) o.label,
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(label, style: AidogType.micro.copyWith(color: theme.c.fg3)),
        const SizedBox(height: AidogSpace.sxs),
        if (options.isEmpty)
          Text(
            emptyLabel,
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          )
        else
          PopupMenuButton<void>(
            tooltip: '',
            position: PopupMenuPosition.under,
            constraints: const BoxConstraints(maxHeight: 280, minWidth: 220),
            color: theme.c.surface2,
            itemBuilder: (context) => [
              // 整张清单塞进**一个** item：`PopupMenuItem` 一点就关，
              // 多选要能连点几下，所以勾选态由这里的 StatefulBuilder 自己管，
              // 同时把每次点击透传给外层。
              PopupMenuItem<void>(
                enabled: false,
                padding: EdgeInsets.zero,
                child: StatefulBuilder(
                  builder: (context, setLocal) {
                    final local = [...selected];
                    return SingleChildScrollView(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          for (final o in options)
                            InkWell(
                              key: itemKeyPrefix == null
                                  ? null
                                  : ValueKey('$itemKeyPrefix-${o.value}'),
                              onTap: () {
                                onToggle(o.value);
                                setLocal(() {
                                  local.contains(o.value)
                                      ? local.remove(o.value)
                                      : local.add(o.value);
                                });
                              },
                              child: Padding(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: AidogSpace.ssm,
                                  vertical: 6,
                                ),
                                child: Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    Icon(
                                      local.contains(o.value)
                                          ? Icons.check_box
                                          : Icons.check_box_outline_blank,
                                      size: 14,
                                      color: local.contains(o.value)
                                          ? theme.c.accentText
                                          : theme.c.fg3,
                                    ),
                                    const SizedBox(width: AidogSpace.sxs),
                                    Flexible(
                                      child: Text(
                                        o.label,
                                        overflow: TextOverflow.ellipsis,
                                        style: AidogType.micro.copyWith(
                                          color: theme.c.fg,
                                        ),
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                        ],
                      ),
                    );
                  },
                ),
              ),
            ],
            child: Container(
              padding: const EdgeInsets.symmetric(
                horizontal: AidogSpace.ssm,
                vertical: 5,
              ),
              decoration: BoxDecoration(
                border: Border.all(color: theme.c.line),
                borderRadius: BorderRadius.circular(AidogRadius.sm),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Flexible(
                    child: Text(
                      chosen.isEmpty ? emptyLabel : chosen.join('、'),
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(
                        color: chosen.isEmpty ? theme.c.fg3 : theme.c.fg,
                      ),
                    ),
                  ),
                  Icon(Icons.arrow_drop_down, size: 16, color: theme.c.fg3),
                ],
              ),
            ),
          ),
        const SizedBox(height: AidogSpace.ssm),
      ],
    );
  }
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
          FieldLabel(label),
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
    this.labelIcon,
    required this.options,
    required this.value,
    required this.onChanged,
    this.labelOf,
    this.description,
    this.trailing,
  });

  /// 下拉右侧的附加按钮（环境变量编辑器的「移除」× 就挂这里）。
  final Widget? trailing;

  /// 标签行首的图标（同 [TextRow.labelIcon]）。
  final IconData? labelIcon;

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
          FieldLabel(label, icon: labelIcon),
          if (description != null && description!.isNotEmpty)
            Text(
              description!,
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          Row(
            children: [
              Expanded(
                child: DropdownButton<String>(
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
              ),
              if (trailing != null) ...[
                const SizedBox(width: AidogSpace.sxs),
                trailing!,
              ],
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
  const ErrorNote({super.key, required this.text, this.action});

  final String text;

  /// 右侧的补救动作（如代理起不来时的「重试」）。报了错却没有出口，
  /// 用户只能去别处找按钮（`ProxyStatusSection.tsx:97-99`）。
  final Widget? action;

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
        child: Row(
          children: [
            Expanded(
              child: Text(
                text,
                style: AidogType.micro.copyWith(color: theme.c.bad),
              ),
            ),
            if (action != null) ...[
              const SizedBox(width: AidogSpace.ssm),
              action!,
            ],
          ],
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
    // React 侧是普通 `Dialog` + createPortal（`UnsavedChangesModal.tsx:30`，
    // maxWidth 420）：点遮罩等于「取消离开」，执行中不许关。
    return AidogModal(
      onBarrierTap: busy ? null : onCancel,
      child: ModalCard(
        // `DialogContent` 自带 ✕（保存中不给关，与遮罩同口径）。
        onClose: busy ? null : onCancel,
        // `padding: "22px 24px"`、标题 F.title 20 w600
        //（`UnsavedChangesModal.tsx:31-35`）。
        padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 22),
        titleStyle: AidogType.title.copyWith(
          fontSize: 20,
          fontWeight: FontWeight.w600,
        ),
        title: t.t('settings.unsavedTitle'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              // 正文 F.body = 15、行高 1.6（`UnsavedChangesModal.tsx:36`）。
              t.t('settings.unsavedBody'),
              style: AidogType.body.copyWith(height: 1.6, color: theme.c.fg2),
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
///
/// `maxLines: null` = **随内容自增高**（对齐 React 的 `AutoTextarea`）。长正则和
/// 多行值靠它才看得全。自增高做在这里而不是给每处各写一个组件：中间件那边
/// 光条件树 pattern + 动作链 replacement / value / override body 就有 9 处。
class PlainTextField extends StatefulWidget {
  const PlainTextField({
    super.key,
    required this.value,
    this.onChanged,
    this.onSubmitted,
    this.hint,
    this.maxLines = 1,
    this.minLines,
    this.enabled = true,
    this.obscure = false,
    this.mono = false,
  });

  final String value;
  final String? hint;
  final ValueChanged<String>? onChanged;
  final ValueChanged<String>? onSubmitted;

  /// `null` = 自增高，不封顶。
  final int? maxLines;

  /// 自增高模式的起始行数（模板框那种要直接给出足够高度）。
  final int? minLines;
  final bool enabled;

  /// 遮挡显示（密码 / 令牌，React `<input type="password">` 无明文切换）。
  final bool obscure;

  /// 等宽字（模板 / 命令这类内容缩进对不齐就看不出层级）。
  final bool mono;

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
      maxLines: widget.obscure ? 1 : widget.maxLines,
      obscureText: widget.obscure,
      // 自增高那条路要给 minLines，否则首帧就按 1 行高度画完再跳。
      minLines: widget.obscure || widget.maxLines == 1
          ? null
          : widget.minLines ?? 1,
      // 单行时回车提交；多行时回车是换行，提交交给失焦（上面的 FocusNode 监听）。
      keyboardType: widget.maxLines == 1
          ? TextInputType.text
          : TextInputType.multiline,
      style: (widget.mono ? AidogType.numSm : AidogType.label).copyWith(
        color: widget.enabled ? theme.c.fg : theme.c.fg3,
      ),
      decoration: InputDecoration(
        isDense: true,
        hintText: widget.hint,
        hintStyle: (widget.mono ? AidogType.numSm : AidogType.label)
            .copyWith(color: theme.c.fg3),
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
      // 设置页页头不是 `.section-title`：React 各设置页写的是
      // `fontSize: F.title`(20) + w600（`CodexSettings.tsx:165`、
      // `PiSettings.tsx` 同构）。
      PageHead(
        title: title,
        subtitle: subtitle,
        trailing: trailing,
        titleStyle: AidogType.display.copyWith(
          fontSize: 20,
          fontWeight: FontWeight.w600,
          letterSpacing: 0,
        ),
      ),
      // 区块逐个错峰淡入（React 各设置页给每张卡传 `staggerMs`，
      // 如 `CodingToolsSettings.tsx:376,384,394,408,420`；步长取
      // `CodexSettings.tsx:240` / `PiSettings.tsx:242` 的 idx * 60）。
      // 这里在容器层统一按下标错峰，页面不必逐张传值 —— 顺序就是下标，不会漏也不会重。
      for (final (i, c) in children.indexed) Reveal(delayMs: i * 60, child: c),
    ],
  );
}
