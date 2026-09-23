/// 带自动补全的路径输入行（票 I19b，命令覆盖缺口 C6）。
///
/// React 真值源：`src/components/settings/editors/_shared.tsx:541-745`（`PathInput`），
/// 由 `FieldRenderer.tsx:174` 挂在所有带 `pathType` 的 schema 字段上。
///
/// 行为逐条对齐：150 ms 防抖 → `fs_autocomplete` → 下拉候选；↑/↓ 循环高亮、
/// Tab 选中（无高亮时取第一条）、Enter 仅在有高亮时选中、Esc 收起；失焦 200 ms 后收起；
/// 候选为空不弹；选中目录会补 `/` 并就地再查一次（可继续往下钻），选中文件则收起。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../i18n.dart';
import '../../../platform.dart';
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../invoke.dart';

/// 一条候选。字段名照抄 Rust 的 `PathEntry`
/// （`src-tauri/crates/aidog_core/src/system_cmd/fs_autocomplete.rs:4`）。
class PathSuggestion {
  const PathSuggestion({
    required this.name,
    required this.fullPath,
    required this.isDir,
    required this.modified,
  });

  final String name;
  final String fullPath;
  final bool isDir;

  /// Unix 秒。
  final int modified;

  static PathSuggestion fromJson(Map<String, Object?> j) => PathSuggestion(
    name: '${j['name'] ?? ''}',
    fullPath: '${j['full_path'] ?? ''}',
    isDir: j['is_dir'] == true,
    modified: (j['modified'] as num?)?.toInt() ?? 0,
  );
}

/// 候选的修改时间 → 相对时间串。逐条照抄 React 的 `formatTime`。
String formatSuggestionTime(I18nController t, int ts, {DateTime? now}) {
  if (ts == 0) return '';
  final d = DateTime.fromMillisecondsSinceEpoch(ts * 1000);
  final n = now ?? DateTime.now();
  final diffMs = n.difference(d).inMilliseconds;
  final diffDays = (diffMs / 86400000).floor();
  if (diffDays == 0) {
    final diffH = (diffMs / 3600000).floor();
    return diffH == 0
        ? t.t('settings.editor.justNow')
        : '$diffH${t.t('settings.editor.hoursAgo')}';
  }
  if (diffDays < 30) return '$diffDays${t.t('settings.editor.daysAgo')}';
  return '${d.year}-${'${d.month}'.padLeft(2, '0')}-${'${d.day}'.padLeft(2, '0')}';
}

/// 补全下拉里的按键意图。Flutter 的 `Shortcuts` 需要一个 Intent 类型；
/// 五个键共用一个带枚举的 Intent，不为每个键造一个类。
enum PathKey { down, up, tab, enter, escape }

class _PathKeyIntent extends Intent {
  const _PathKeyIntent(this.key);
  final PathKey key;
}

/// 带补全的路径输入行。
class PathInputRow extends StatefulWidget {
  const PathInputRow({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    required this.pathType,
    this.description,
    this.hint,
    this.invoke = kernelInvoke,
    this.pick,
    this.showPicker = true,
  });

  final String label;
  final String? description;
  final String? hint;
  final String? value;

  /// 空串回传 `null`（与 React 的 `e.target.value || undefined` 同语义）。
  final ValueChanged<String?> onChanged;

  /// `file` | `directory`
  final String pathType;
  final InvokeFn invoke;

  /// 系统文件对话框（测试注入）。
  final Future<String?> Function(bool directory)? pick;

  /// 测试里关掉原生对话框按钮。
  final bool showPicker;

  @override
  State<PathInputRow> createState() => _PathInputRowState();
}

class _PathInputRowState extends State<PathInputRow> {
  late final TextEditingController _ctrl = TextEditingController(
    text: widget.value ?? '',
  );
  final FocusNode _focus = FocusNode();

  List<PathSuggestion> _suggestions = const [];
  bool _show = false;
  int _hl = -1;
  Timer? _debounce;
  Timer? _blur;

  bool get _isDirPicker => widget.pathType == 'directory';

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      if (_focus.hasFocus) {
        if (_suggestions.isNotEmpty) setState(() => _show = true);
      } else {
        _blur?.cancel();
        _blur = Timer(const Duration(milliseconds: 200), () {
          if (mounted) setState(() => _show = false);
        });
      }
    });
  }

  @override
  void didUpdateWidget(PathInputRow old) {
    super.didUpdateWidget(old);
    final v = widget.value ?? '';
    if (v != _ctrl.text && !_focus.hasFocus) {
      _ctrl.value = TextEditingValue(
        text: v,
        selection: TextSelection.collapsed(offset: v.length),
      );
    }
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _blur?.cancel();
    _focus.dispose();
    _ctrl.dispose();
    super.dispose();
  }

  void _fetch(String input) {
    _debounce?.cancel();
    if (input.isEmpty) {
      setState(() {
        _suggestions = const [];
        _show = false;
      });
      return;
    }
    _debounce = Timer(const Duration(milliseconds: 150), () async {
      try {
        final r = await widget.invoke('fs_autocomplete', {'input': input});
        if (!mounted) return;
        final list = (r as List? ?? const [])
            .whereType<Map>()
            .map((e) => PathSuggestion.fromJson(Map<String, Object?>.from(e)))
            .toList();
        setState(() {
          _suggestions = list;
          _show = list.isNotEmpty;
          _hl = -1;
        });
      } catch (_) {
        if (!mounted) return;
        setState(() {
          _suggestions = const [];
          _show = false;
        });
      }
    });
  }

  void _setText(String v) {
    _ctrl.value = TextEditingValue(
      text: v,
      selection: TextSelection.collapsed(offset: v.length),
    );
    widget.onChanged(v.isEmpty ? null : v);
  }

  /// 选目录时补 `/` 并就地再查一次，方便继续往下钻；选文件直接收起。
  void _select(PathSuggestion s) {
    if (s.isDir) {
      _setText('${s.fullPath}/');
      _fetch('${s.fullPath}/');
    } else {
      _setText(s.fullPath);
      setState(() => _show = false);
    }
  }

  void _onKey(PathKey k) {
    if (!_show || _suggestions.isEmpty) return;
    switch (k) {
      case PathKey.down:
        setState(() => _hl = (_hl + 1) % _suggestions.length);
      case PathKey.up:
        setState(() => _hl = _hl <= 0 ? _suggestions.length - 1 : _hl - 1);
      case PathKey.tab:
        _select(_suggestions[_hl >= 0 ? _hl : 0]);
      case PathKey.enter:
        if (_hl >= 0) _select(_suggestions[_hl]);
      case PathKey.escape:
        setState(() => _show = false);
    }
  }

  Future<void> _pick() async {
    final picker =
        widget.pick ??
        (bool dir) =>
            pickPath(PickPathOptions(directory: dir)).then((v) => v);
    final selected = await picker(_isDirPicker);
    if (selected != null && selected.isNotEmpty) _setText(selected);
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final value = _ctrl.text;
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
          Row(
            children: [
              Expanded(child: _field(t, theme)),
              if (widget.showPicker)
                IconButton(
                  key: const ValueKey('path-pick'),
                  icon: Icon(Icons.folder_open, size: 15, color: theme.c.fg2),
                  tooltip: _isDirPicker
                      ? t.t('settings.editor.chooseDir')
                      : t.t('settings.editor.chooseFile'),
                  onPressed: _pick,
                ),
            ],
          ),
          if (_show && _suggestions.isNotEmpty) _dropdown(t, theme),
          if (value.isEmpty && !_show)
            Text(
              _isDirPicker
                  ? t.t('settings.editor.dirHint')
                  : t.t('settings.editor.fileHint'),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
        ],
      ),
    );
  }

  Widget _field(I18nController t, AidogTheme theme) => Shortcuts(
    shortcuts: const {
      SingleActivator(LogicalKeyboardKey.arrowDown): _PathKeyIntent(PathKey.down),
      SingleActivator(LogicalKeyboardKey.arrowUp): _PathKeyIntent(PathKey.up),
      SingleActivator(LogicalKeyboardKey.tab): _PathKeyIntent(PathKey.tab),
      SingleActivator(LogicalKeyboardKey.enter): _PathKeyIntent(PathKey.enter),
      SingleActivator(LogicalKeyboardKey.escape): _PathKeyIntent(PathKey.escape),
    },
    child: Actions(
      actions: {
        _PathKeyIntent: CallbackAction<_PathKeyIntent>(
          onInvoke: (i) {
            _onKey(i.key);
            return null;
          },
        ),
      },
      child: TextField(
        key: const ValueKey('path-input'),
        controller: _ctrl,
        focusNode: _focus,
        style: AidogType.micro.copyWith(color: theme.c.fg),
        decoration: InputDecoration(
          isDense: true,
          hintText:
              widget.hint ??
              (_isDirPicker
                  ? t.t('settings.editor.dirOrInputPh')
                  : t.t('settings.editor.fileOrInputPh')),
          hintStyle: AidogType.micro.copyWith(color: theme.c.fg3),
        ),
        onChanged: (v) {
          widget.onChanged(v.isEmpty ? null : v);
          _fetch(v);
        },
      ),
    ),
  );

  Widget _dropdown(I18nController t, AidogTheme theme) => Container(
    key: const ValueKey('path-suggestions'),
    margin: const EdgeInsets.only(top: 2),
    constraints: const BoxConstraints(maxHeight: 240),
    decoration: BoxDecoration(
      color: theme.c.surface2,
      border: Border.all(color: theme.c.line),
      borderRadius: BorderRadius.circular(AidogRadius.sm),
    ),
    child: SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var i = 0; i < _suggestions.length; i++)
            _row(t, theme, _suggestions[i], i),
        ],
      ),
    ),
  );

  Widget _row(
    I18nController t,
    AidogTheme theme,
    PathSuggestion s,
    int i,
  ) => InkWell(
    key: ValueKey('path-sugg-$i'),
    onTap: () => _select(s),
    child: Container(
      // 键盘高亮行：淡底 + 左侧亮竖条。只有淡底的话对表面只有 1.15:1，
      // 到不了 1.4.11 要求的 3:1 —— 键盘用户看不出光标停在哪一行。
      // 透明边常驻，高亮时才上色，避免行宽在高亮切换时跳。
      decoration: BoxDecoration(
        color: i == _hl ? theme.c.accentWash : null,
        border: Border(
          left: BorderSide(
            color: i == _hl ? theme.c.accentEdge : Colors.transparent,
            width: 2,
          ),
        ),
      ),
      padding: const EdgeInsets.symmetric(
        horizontal: AidogSpace.ssm,
        vertical: AidogSpace.sxs,
      ),
      child: Row(
        children: [
          Icon(
            s.isDir ? Icons.folder : Icons.insert_drive_file_outlined,
            size: 13,
            color: theme.c.fg3,
          ),
          const SizedBox(width: AidogSpace.sxs),
          Expanded(
            child: Text(
              s.name,
              style: AidogType.micro.copyWith(color: theme.c.fg),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(width: AidogSpace.sxs),
          Text(
            formatSuggestionTime(t, s.modified),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
        ],
      ),
    ),
  );
}
