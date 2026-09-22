/// statusline / subagent-statusline 配置面板（票 I19b，命令覆盖缺口 C5）。
///
/// React 真值源：
/// - `src/components/settings/editors/StatusLineSection.tsx`（外层：两个面板 +
///   fileSuggestion 字段 + 数据字段参考表）
/// - `src/components/settings/editors/StatusLineSection/useStatusLinePanel.ts`（状态层）
/// - 同目录 `StatusLinePanel.tsx` / `SegmentEditModal.tsx` / `preview.tsx`
///
/// 纯计算（段列表的增删改 / 分行 / 上色）全在 [statusline_model.dart]，本文件只画界面。
///
/// 两处与 React 的形态差异（与本仓库既有约定一致，不是遗漏）：
/// 1. 段编辑器是**页面内的卡片**而不是 Portal 弹窗 —— 与 `ImportDiffCard` /
///    `UnsavedChangesCard` 同一条规矩（`ui_bits.dart:64` 写明理由）。
/// 2. 排序用 [ReorderableListView] 的长按手柄，而不是 dnd-kit 的自定义 handle。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../../utils/color_level.dart';
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'statusline_model.dart';

/// 段名 / 段说明 / 段字段的取词，缺 key 回落到定义表里的中文字面量（与 React 的
/// `t(key, fallback)` 同语义）。
String segName(I18nController t, SegmentDef d) =>
    tOr(t, 'statusline.seg.${d.type}.name', d.name);
String segDesc(I18nController t, SegmentDef d) =>
    tOr(t, 'statusline.seg.${d.type}.desc', d.desc);
String segFieldLabel(I18nController t, SegmentDef d, SegmentField f) =>
    tOr(t, 'statusline.seg.${d.type}.field.${f.key}', f.label);

const _mono = TextStyle(fontFamily: AidogType.familyMono);

/// 一个 statusline 面板（主状态栏 / 子代理状态栏共用一棵树）。
class StatusLinePanel extends StatefulWidget {
  const StatusLinePanel({
    super.key,
    required this.config,
    required this.updateField,
    required this.scriptType,
    this.invoke = kernelInvoke,
    this.now,
  });

  final Map<String, Object?> config;
  final void Function(String field, Object? value) updateField;

  /// `statusline` | `subagent`
  final String scriptType;
  final InvokeFn invoke;

  /// 新段 id 的时间源（测试注入，保证 id 稳定）。
  final int Function()? now;

  @override
  State<StatusLinePanel> createState() => _StatusLinePanelState();
}

class _StatusLinePanelState extends State<StatusLinePanel> {
  bool _showScript = false;
  bool _showAddMenu = false;
  String _scriptPreview = '';
  String? _previewKey;

  /// 正在编辑的段 id（null = 没开编辑器）。
  String? _editSegId;

  bool get _isMain => widget.scriptType == 'statusline';
  String get _aidogKey =>
      _isMain ? '_aidog_statusline' : '_aidog_subagent_statusline';
  String get _fieldName => _isMain ? 'statusLine' : 'subagentStatusLine';

  Map<String, Object?> get _stored => widget.config[_aidogKey] is Map
      ? Map<String, Object?>.from(widget.config[_aidogKey]! as Map)
      : <String, Object?>{};

  bool get _enabled => _stored['enabled'] == true;
  String get _mode => _stored['mode'] == 'custom' ? 'custom' : 'builtin';
  String get _customCommand =>
      _stored['customCommand'] is String ? _stored['customCommand']! as String : '';

  List<StatusLineSegment> get _defaultSegments =>
      _isMain ? kDefaultSegments : kDefaultSubagentSegments;

  List<StatusLineSegment> get _segments {
    final raw = _stored['segments'];
    if (raw is List) {
      return raw
          .whereType<Map>()
          .map((e) => StatusLineSegment.fromJson(Map<String, Object?>.from(e)))
          .toList();
    }
    return [..._defaultSegments];
  }

  void _setStored(Map<String, Object?> patch) {
    widget.updateField(_aidogKey, {..._stored, ...patch});
  }

  void _updateSegments(List<StatusLineSegment> next) {
    _setStored({
      'segments': normalizeSegments(next).map((s) => s.toJson()).toList(),
    });
  }

  void _handleToggle(bool val) {
    if (!val) {
      widget.updateField(_fieldName, null);
      _setStored({'enabled': false});
    } else {
      _setStored({'enabled': true});
    }
  }

  /// 切生成模式。清掉原生字段，两种模式不留下对方的残值（用户在新模式里重新「应用」）。
  void _switchMode(String next) {
    if (next == _mode) return;
    widget.updateField(_fieldName, null);
    _setStored({'mode': next});
  }

  /// 自定义模式：直接写原生 `statusLine.command`，不生成 aidog 脚本。空命令清字段。
  void _applyCustom() {
    final cmd = _customCommand.trim();
    if (cmd.isEmpty) {
      widget.updateField(_fieldName, null);
      return;
    }
    widget.updateField(_fieldName, {'type': 'command', 'command': cmd});
  }

  String _newId() =>
      's${(widget.now ?? () => DateTime.now().millisecondsSinceEpoch)()}';

  void _addSegment(String type, {bool newline = false}) {
    final seg = makeSegment(type, newline: newline, id: _newId());
    if (seg == null) return;
    _updateSegments([..._segments, seg]);
    setState(() => _showAddMenu = false);
  }

  void _addRow() => _addSegment('model', newline: _segments.isNotEmpty);

  void _resetToDefaultLayout() {
    _setStored({
      'segments': _defaultSegments.map((s) => s.toJson()).toList(),
    });
  }

  /// 脚本预览由 Rust 渲染（与落盘时是同一个生成器），所以看到的就是磁盘上的那份。
  /// 只读：真正的 `.py` 由 `do_sync_group_settings` 写，永远不从这里写。
  void _syncPreview() {
    if (!_showScript || !_enabled || _mode != 'builtin') return;
    final key = jsonEncode(_stored);
    if (key == _previewKey) return;
    _previewKey = key;
    unawaited(
      widget
          .invoke('preview_statusline_script', {
            'scriptType': widget.scriptType,
            'stored': _stored,
          })
          .then((src) {
            if (mounted) setState(() => _scriptPreview = '${src ?? ''}');
          })
          .catchError((Object _) {
            // React 侧同样只 console.error，不弹错误 —— 预览失败不阻断编辑。
            return null;
          }),
    );
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    _syncPreview();
    final theme = AidogTheme.of(context);
    final segments = _segments;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        SwitchRow(
          key: ValueKey('sl-${widget.scriptType}-enable'),
          label: _isMain
              ? t.t('statusline.useBuiltin')
              : t.t('statusline.useBuiltinSubagent'),
          description: _isMain
              ? t.t('statusline.builtinDesc')
              : t.t('statusline.builtinSubagentDesc'),
          value: _enabled,
          onChanged: _handleToggle,
        ),
        if (_enabled)
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
            child: Text(
              '● ${t.t('statusline.enabled')}',
              style: AidogType.micro.copyWith(color: theme.c.ok),
            ),
          ),
        if (_enabled)
          Row(
            children: [
              for (final m in const ['builtin', 'custom'])
                Padding(
                  padding: const EdgeInsets.only(right: AidogSpace.ssm),
                  child: SmallButton(
                    key: ValueKey('sl-${widget.scriptType}-mode-$m'),
                    label: m == 'builtin'
                        ? t.t('statusline.modeBuiltin')
                        : t.t('statusline.modeCustom'),
                    active: _mode == m,
                    onTap: () => _switchMode(m),
                  ),
                ),
            ],
          ),
        if (_enabled && _mode == 'custom') ..._customMode(t),
        if (_enabled && _mode == 'builtin') ..._builtinMode(t, theme, segments),
      ],
    );
  }

  List<Widget> _customMode(I18nController t) => [
    Padding(
      padding: const EdgeInsets.only(top: AidogSpace.ssm),
      child: Text(
        t.t('statusline.customDesc'),
        style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg3),
      ),
    ),
    InfoRow(label: t.t('statusline.customType'), value: 'command'),
    TextRow(
      key: ValueKey('sl-${widget.scriptType}-custom-cmd'),
      label: t.t('statusline.customCommand'),
      description: t.t('statusline.customCommandDesc'),
      hint: t.t('statusline.customPlaceholder'),
      value: _customCommand,
      onChanged: (v) => _setStored({'customCommand': v}),
    ),
    Align(
      alignment: AlignmentDirectional.centerEnd,
      child: SmallButton(
        key: ValueKey('sl-${widget.scriptType}-apply-custom'),
        label: t.t('statusline.applyCustom'),
        onTap: _applyCustom,
      ),
    ),
  ];

  List<Widget> _builtinMode(
    I18nController t,
    AidogTheme theme,
    List<StatusLineSegment> segments,
  ) => [
    const SizedBox(height: AidogSpace.ssm),
    TileMeta(t.t('statusline.preview')),
    StatusLinePreview(
      key: ValueKey('sl-${widget.scriptType}-preview'),
      segments: segments,
      empty: t.t('statusline.previewEmpty'),
    ),
    const SizedBox(height: AidogSpace.smd),
    _SegmentList(
      scriptType: widget.scriptType,
      segments: segments,
      onReorder: (from, to) {
        final next = [...segments];
        next.insert(to, next.removeAt(from));
        _updateSegments(next);
      },
      onToggle: (id, v) => _updateSegments([
        for (final s in segments)
          if (s.id == id) s.copyWith(enabled: v) else s,
      ]),
      onToggleNewline: (id) => _updateSegments([
        for (final s in segments)
          if (s.id == id) s.copyWith(newline: !s.newline) else s,
      ]),
      onEdit: (id) => setState(() => _editSegId = id),
      onDelete: (id) =>
          _updateSegments(segments.where((s) => s.id != id).toList()),
      onCycleAlign: (id) => _updateSegments(cycleRowAlign(segments, id)),
      onDeleteRow: (id) => _updateSegments(deleteRow(segments, id)),
    ),
    Row(
      children: [
        Tooltip(
          message: t.t('statusline.resetLayoutHint'),
          child: SmallButton(
            key: ValueKey('sl-${widget.scriptType}-reset-layout'),
            label: t.t('statusline.resetLayout'),
            onTap: _resetToDefaultLayout,
          ),
        ),
        const Spacer(),
        SmallButton(
          key: ValueKey('sl-${widget.scriptType}-add-row'),
          label: t.t('statusline.addRow'),
          onTap: _addRow,
        ),
        const SizedBox(width: AidogSpace.ssm),
        SmallButton(
          key: ValueKey('sl-${widget.scriptType}-add-segment'),
          label: t.t('statusline.addSegment'),
          active: _showAddMenu,
          onTap: () => setState(() => _showAddMenu = !_showAddMenu),
        ),
      ],
    ),
    if (_showAddMenu)
      _AddSegmentMenu(
        key: ValueKey('sl-${widget.scriptType}-add-menu'),
        onPick: (type) => _addSegment(type),
      ),
    const SizedBox(height: AidogSpace.ssm),
    Align(
      alignment: AlignmentDirectional.centerStart,
      child: SmallButton(
        key: ValueKey('sl-${widget.scriptType}-toggle-script'),
        label:
            '${_showScript ? '▾' : '▸'} ${t.t('statusline.scriptPreview')}  '
            '~/.aidog/scripts/aidog-'
            '${widget.scriptType == 'subagent' ? 'subagent-' : ''}statusline.py',
        active: _showScript,
        onTap: () => setState(() => _showScript = !_showScript),
      ),
    ),
    if (_showScript)
      Container(
        key: ValueKey('sl-${widget.scriptType}-script-body'),
        width: double.infinity,
        margin: const EdgeInsets.only(top: AidogSpace.sxs),
        padding: const EdgeInsets.all(AidogSpace.smd),
        decoration: BoxDecoration(
          color: theme.c.surface2,
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        constraints: const BoxConstraints(maxHeight: 320),
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: SingleChildScrollView(
            child: Text(
              _scriptPreview,
              style: AidogType.numSm.copyWith(color: theme.c.fg),
            ),
          ),
        ),
      ),
    if (_editSegId != null)
      () {
        final seg = segments.where((s) => s.id == _editSegId).firstOrNull;
        if (seg == null) return const SizedBox.shrink();
        return SegmentEditCard(
          key: ValueKey('sl-${widget.scriptType}-edit'),
          segment: seg,
          isRowLeader: isRowLeaderSeg(segments, seg.id),
          onCancel: () => setState(() => _editSegId = null),
          onSave: (patched) {
            final idx = segments.indexWhere((s) => s.id == seg.id);
            if (idx >= 0) {
              final next = [...segments];
              next[idx] = patched;
              _updateSegments(next);
            }
            setState(() => _editSegId = null);
          },
        );
      }(),
  ];
}

/// 彩色、按行分组、带对齐的实时预览。
class StatusLinePreview extends StatelessWidget {
  const StatusLinePreview({
    super.key,
    required this.segments,
    required this.empty,
  });

  final List<StatusLineSegment> segments;
  final String empty;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final active = segments.where((s) => s.enabled).toList();
    if (active.isEmpty) {
      return Text(empty, style: AidogType.numSm.copyWith(color: theme.c.fg3));
    }
    final rows = groupRows(active);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final row in rows)
          Row(
            mainAxisAlignment: switch (row.align) {
              RowAlign.center => MainAxisAlignment.center,
              RowAlign.right => MainAxisAlignment.end,
              RowAlign.left => MainAxisAlignment.start,
            },
            children: [
              Flexible(
                child: Text.rich(
                  TextSpan(
                    children: [
                      for (final seg in row.segs)
                        if (kSegmentDefMap[seg.type] case final def?)
                          TextSpan(
                            text: _segPreviewText(def, seg),
                            style: _mono.copyWith(
                              color: previewColor(seg, theme.c) ?? theme.c.fg,
                            ),
                          ),
                    ],
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
      ],
    );
  }
}

/// `affixPre + toPreview + affixSuf`（affix 只在是字符串时参与，与 React 同规则）。
String _segPreviewText(SegmentDef def, StatusLineSegment seg) {
  final opts = effectiveOptions(def, seg);
  final pre = opts['affixPre'] is String ? opts['affixPre']! as String : '';
  final suf = opts['affixSuf'] is String ? opts['affixSuf']! as String : '';
  return '$pre${def.toPreview(opts)}$suf';
}

/// 可拖拽排序的段列表。
class _SegmentList extends StatelessWidget {
  const _SegmentList({
    required this.scriptType,
    required this.segments,
    required this.onReorder,
    required this.onToggle,
    required this.onToggleNewline,
    required this.onEdit,
    required this.onDelete,
    required this.onCycleAlign,
    required this.onDeleteRow,
  });

  final String scriptType;
  final List<StatusLineSegment> segments;
  final void Function(int from, int to) onReorder;
  final void Function(String id, bool value) onToggle;
  final void Function(String id) onToggleNewline;
  final void Function(String id) onEdit;
  final void Function(String id) onDelete;
  final void Function(String id) onCycleAlign;
  final void Function(String id) onDeleteRow;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    if (segments.isEmpty) {
      return CenteredNote(text: t.t('statusline.previewEmpty'));
    }
    return ReorderableListView.builder(
      shrinkWrap: true,
      buildDefaultDragHandles: false,
      physics: const NeverScrollableScrollPhysics(),
      itemCount: segments.length,
      // `onReorderItem` 的 newIndex 已按「先摘掉 oldIndex」修正过，调用方直接 insert。
      onReorderItem: onReorder,
      itemBuilder: (context, i) {
        final seg = segments[i];
        final def = kSegmentDefMap[seg.type];
        if (def == null) {
          return SizedBox.shrink(key: ValueKey('sl-$scriptType-seg-${seg.id}'));
        }
        final leader = isRowLeaderSeg(segments, seg.id);
        final segColor = previewColor(seg, theme.c);
        return Column(
          key: ValueKey('sl-$scriptType-seg-${seg.id}'),
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            if (leader)
              Row(
                children: [
                  Text(
                    t.t('statusline.rowLabel'),
                    style: AidogType.micro.copyWith(color: theme.c.fg3),
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                  SmallButton(
                    key: ValueKey('sl-$scriptType-align-${seg.id}'),
                    label: t.t(
                      'statusline.align.${rowAlignName(seg.align ?? RowAlign.left)}',
                    ),
                    onTap: () => onCycleAlign(seg.id),
                  ),
                  const SizedBox(width: AidogSpace.sxs),
                  SmallButton(
                    key: ValueKey('sl-$scriptType-delrow-${seg.id}'),
                    label: t.t('statusline.deleteRow'),
                    onTap: () => onDeleteRow(seg.id),
                  ),
                ],
              ),
            Opacity(
              opacity: seg.enabled ? 1 : 0.45,
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: AidogSpace.sxs),
                child: Row(
                  children: [
                    ReorderableDragStartListener(
                      index: i,
                      child: Tooltip(
                        message: t.t('statusline.dragSort'),
                        child: Icon(
                          Icons.drag_handle,
                          size: 15,
                          color: theme.c.fg3,
                        ),
                      ),
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                    Switch(
                      key: ValueKey('sl-$scriptType-on-${seg.id}'),
                      value: seg.enabled,
                      onChanged: (v) => onToggle(seg.id, v),
                      activeThumbColor: theme.c.accent,
                      inactiveTrackColor: theme.c.surface2,
                      inactiveThumbColor: theme.c.fg3,
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                    Flexible(
                      child: Text(
                        segName(t, def),
                        style: AidogType.label.copyWith(color: theme.c.fg),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    const SizedBox(width: AidogSpace.ssm),
                    Expanded(
                      child: Text(
                        def.toPreview(effectiveOptions(def, seg)),
                        style: _mono.copyWith(
                          fontSize: 11,
                          color: segColor ?? theme.c.fg3,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    Tooltip(
                      message: t.t('statusline.toggleNewline'),
                      child: SmallButton(
                        key: ValueKey('sl-$scriptType-nl-${seg.id}'),
                        label: '↵',
                        active: seg.newline,
                        onTap: () => onToggleNewline(seg.id),
                      ),
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                    SmallButton(
                      key: ValueKey('sl-$scriptType-edit-${seg.id}'),
                      label: t.t('action.edit'),
                      onTap: () => onEdit(seg.id),
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                    SmallButton(
                      key: ValueKey('sl-$scriptType-del-${seg.id}'),
                      label: '×',
                      onTap: () => onDelete(seg.id),
                    ),
                  ],
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

/// 「添加段」分类选择器。
class _AddSegmentMenu extends StatelessWidget {
  const _AddSegmentMenu({super.key, required this.onPick});

  final void Function(String type) onPick;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Container(
      margin: const EdgeInsets.only(top: AidogSpace.sxs),
      padding: const EdgeInsets.all(AidogSpace.ssm),
      constraints: const BoxConstraints(maxHeight: 360),
      decoration: BoxDecoration(
        color: theme.c.surface2,
        borderRadius: BorderRadius.circular(AidogRadius.md),
        border: Border.all(color: theme.c.line),
      ),
      child: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final cat in kSegmentCategories) ...[
              Padding(
                padding: const EdgeInsets.only(top: AidogSpace.sxs),
                child: Text(
                  tOr(t, 'statusline.segCat.${cat.id}', cat.label),
                  style: AidogType.micro.copyWith(color: theme.c.fg3),
                ),
              ),
              for (final type in cat.types)
                if (kSegmentDefMap[type] case final def?)
                  InkWell(
                    key: ValueKey('sl-add-$type'),
                    onTap: () => onPick(type),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 3),
                      child: Row(
                        children: [
                          Text(
                            segName(t, def),
                            style: AidogType.label.copyWith(color: theme.c.fg),
                          ),
                          const SizedBox(width: AidogSpace.ssm),
                          Flexible(
                            child: Text(
                              segDesc(t, def),
                              style: AidogType.micro.copyWith(color: theme.c.fg3),
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
            ],
          ],
        ),
      ),
    );
  }
}

/// 单个段的编辑卡：换行 / 行对齐 / 颜色 / 类型专属字段 / 预览。
class SegmentEditCard extends StatefulWidget {
  const SegmentEditCard({
    super.key,
    required this.segment,
    required this.isRowLeader,
    required this.onSave,
    required this.onCancel,
  });

  final StatusLineSegment segment;
  final bool isRowLeader;
  final void Function(StatusLineSegment patched) onSave;
  final VoidCallback onCancel;

  @override
  State<SegmentEditCard> createState() => _SegmentEditCardState();
}

class _SegmentEditCardState extends State<SegmentEditCard> {
  late Map<String, Object?> _opts;
  late bool _newline;
  late String _color;
  late bool _autoColor;
  late RowAlign _align;

  @override
  void initState() {
    super.initState();
    final def = kSegmentDefMap[widget.segment.type];
    _opts = {...?def?.defaultOptions, ...widget.segment.options};
    _newline = widget.segment.newline;
    _color = widget.segment.color ?? '';
    _autoColor = widget.segment.autoColor == true;
    _align = widget.segment.align ?? RowAlign.left;
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final def = kSegmentDefMap[widget.segment.type];
    if (def == null) return const SizedBox.shrink();
    final canAutoColor = kValueColorable.contains(widget.segment.type);
    final validHex = hexToRgb(_color) != null;
    final effective = _autoColor && canAutoColor
        ? levelColor(autoColorPreviewLevel(widget.segment.type), theme.c)
        : (validHex ? previewColor(
            StatusLineSegment(
              id: widget.segment.id,
              type: widget.segment.type,
              enabled: true,
              newline: false,
              color: _color,
            ),
            theme.c,
          ) : null);

    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: segName(t, def),
        meta: segDesc(t, def),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            SwitchRow(
              key: const ValueKey('seg-edit-newline'),
              label: t.t('statusline.segNewline'),
              value: _newline,
              onChanged: (v) => setState(() => _newline = v),
            ),
            if (widget.isRowLeader || _newline)
              ChoiceRow(
                key: const ValueKey('seg-edit-align'),
                label: t.t('statusline.rowAlign'),
                options: [for (final a in RowAlign.values) a.name],
                labelOf: (o) => t.t('statusline.align.$o'),
                value: _align.name,
                onChanged: (v) =>
                    setState(() => _align = rowAlignFrom(v) ?? RowAlign.left),
              ),
            if (canAutoColor)
              SwitchRow(
                key: const ValueKey('seg-edit-autocolor'),
                label: t.t('statusline.autoColor'),
                value: _autoColor,
                onChanged: (v) => setState(() => _autoColor = v),
              ),
            Row(
              children: [
                Expanded(
                  child: TextRow(
                    key: const ValueKey('seg-edit-color'),
                    label: t.t('statusline.color'),
                    hint: '#4A9EFF',
                    value: _color,
                    // autoColor 接管上色时固定色不可编辑（React 那边是 pointerEvents:none）。
                    onChanged: _autoColor && canAutoColor
                        ? null
                        : (v) => setState(() => _color = v),
                  ),
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  key: const ValueKey('seg-edit-clear-color'),
                  label: t.t('statusline.clearColor'),
                  onTap: _autoColor && canAutoColor
                      ? null
                      : () => setState(() => _color = ''),
                ),
              ],
            ),
            for (final f in def.fields) _fieldRow(t, def, f),
            TileMeta(t.t('statusline.preview')),
            Text(
              def.toPreview(_opts),
              key: const ValueKey('seg-edit-preview'),
              style: _mono.copyWith(color: effective ?? theme.c.fg),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  key: const ValueKey('seg-edit-cancel'),
                  label: t.t('statusline.cancel'),
                  onTap: widget.onCancel,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  key: const ValueKey('seg-edit-save'),
                  label: t.t('statusline.save'),
                  onTap: () => widget.onSave(
                    widget.segment.copyWith(
                      options: _opts,
                      newline: _newline,
                      color: validHex ? _color : null,
                      clearColor: !validHex,
                      autoColor: canAutoColor ? _autoColor : null,
                      clearAutoColor: !canAutoColor,
                      align: (widget.isRowLeader || _newline) ? _align : null,
                      clearAlign: !(widget.isRowLeader || _newline),
                    ),
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _fieldRow(I18nController t, SegmentDef def, SegmentField f) {
    final label = segFieldLabel(t, def, f);
    final key = ValueKey('seg-edit-f-${f.key}');
    switch (f.type) {
      case 'select':
        return ChoiceRow(
          key: key,
          label: label,
          options: f.options,
          value: '${_opts[f.key] ?? (f.options.isEmpty ? '' : f.options.first)}',
          onChanged: (v) => setState(() => _opts = {..._opts, f.key: v}),
        );
      case 'number':
        // React: `Number(e.target.value)` —— 空串 / 非数字进来是 NaN，这里取 0。
        return NumberRow(
          key: key,
          label: label,
          value: (_opts[f.key] is num) ? (_opts[f.key]! as num).toInt() : 0,
          onChanged: (v) => setState(() => _opts = {..._opts, f.key: v}),
        );
      default:
        return TextRow(
          key: key,
          label: label,
          hint: f.placeholder,
          value: '${_opts[f.key] ?? ''}',
          onChanged: (v) => setState(() => _opts = {..._opts, f.key: v}),
        );
    }
  }
}

/// 设置页「状态栏」分区的两个面板（主状态栏 + 子代理状态栏）。
///
/// 对应 React `StatusLineSection.tsx` 的前两块。它后面的 `fileSuggestion` 字段由
/// 通用行渲染器画（schema 里带 `pathType: file` → 走带补全的路径输入行），
/// 再后面是 [StatusLineDataRef] —— 三者的顺序与 React 一致，由调用方拼。
class StatusLineSectionBody extends StatelessWidget {
  const StatusLineSectionBody({
    super.key,
    required this.config,
    required this.updateField,
    this.invoke = kernelInvoke,
  });

  final Map<String, Object?> config;
  final void Function(String field, Object? value) updateField;
  final InvokeFn invoke;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      StatusLinePanel(
        config: config,
        updateField: updateField,
        scriptType: 'statusline',
        invoke: invoke,
      ),
      const SizedBox(height: AidogSpace.slg),
      StatusLinePanel(
        config: config,
        updateField: updateField,
        scriptType: 'subagent',
        invoke: invoke,
      ),
      const SizedBox(height: AidogSpace.slg),
    ],
  );
}

/// 可用数据字段参考（折叠）。
class StatusLineDataRef extends StatefulWidget {
  const StatusLineDataRef({super.key});

  @override
  State<StatusLineDataRef> createState() => _StatusLineDataRefState();
}

class _StatusLineDataRefState extends State<StatusLineDataRef> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        SmallButton(
          key: const ValueKey('sl-dataref-toggle'),
          label: '${_open ? '▾' : '▸'} ${t.t('statusline.dataFieldsRef')}',
          active: _open,
          onTap: () => setState(() => _open = !_open),
        ),
        if (_open) ...[
          Padding(
            padding: const EdgeInsets.symmetric(vertical: AidogSpace.ssm),
            child: Text(
              t.t('statusline.dataFieldsHint'),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
          for (final g in kStatuslineDataFields) ...[
            Text(
              tOr(t, 'statusline.dataGroup.${g.id}', g.group),
              style: AidogType.label.copyWith(color: theme.c.fg2),
            ),
            for (final f in g.fields)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 2),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Flexible(
                      child: Text(
                        f.$1,
                        style: _mono.copyWith(
                          fontSize: 11,
                          color: theme.c.accent,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    const SizedBox(width: AidogSpace.ssm),
                    Flexible(
                      child: Text(
                        f.$2,
                        style: AidogType.micro.copyWith(color: theme.c.fg3),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
              ),
          ],
        ],
      ],
    );
  }
}
