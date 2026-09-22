/// 沙箱配置的可视化编辑器 —— 对齐
/// `src/components/settings/editors/SandboxSection.tsx`。
///
/// 原先这个字段在 Flutter 侧只是一个裸 JSON 编辑框（schema 里 `sandbox` 是
/// `type: json` + `skipGui`），用户要手敲 `filesystem.allowWrite` 这种嵌套结构。
/// 这里把 React 那四块（启用开关 / 文件系统隔离 / 网络隔离 / 安全与策略 / 排除命令）
/// 原样搬过来。
///
/// **写回规则照抄 React 的 `sync`**（`SandboxSection.tsx:140`）：顶层空数组、
/// false、null 一律删键；`filesystem` / `network` 子对象清空后整个删掉；
/// 整棵树空了就把 `sandbox` 写成 null。少删一层就会在 settings.json 里留下
/// `"sandbox": {"filesystem": {}}` 这种噪声。
library;

import 'package:flutter/material.dart';

import '../../../i18n.dart';
import '../../shell/theme.dart';
import '../../shell/tiles.dart';
import '../invoke.dart';
import '../ui_bits.dart';
import 'bits.dart';
import 'path_input.dart';

/// 可增删的字符串列表（纯文本输入，域名 / 命令用）。
/// 对应 React 的 `TagList`（`SandboxSection.tsx:16`）。
class SandboxTagList extends StatefulWidget {
  const SandboxTagList({
    super.key,
    required this.items,
    required this.onChanged,
    this.hint,
  });

  final List<String> items;
  final ValueChanged<List<String>> onChanged;
  final String? hint;

  @override
  State<SandboxTagList> createState() => _SandboxTagListState();
}

class _SandboxTagListState extends State<SandboxTagList> {
  final TextEditingController _draft = TextEditingController();

  @override
  void dispose() {
    _draft.dispose();
    super.dispose();
  }

  /// 空白项与重复项都不加（React：`if (v && !items.includes(v))`）。
  void _add() {
    final v = _draft.text.trim();
    if (v.isEmpty || widget.items.contains(v)) {
      _draft.clear();
      setState(() {});
      return;
    }
    widget.onChanged([...widget.items, v]);
    _draft.clear();
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (var i = 0; i < widget.items.length; i++)
          Padding(
            padding: const EdgeInsets.only(bottom: 2),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    widget.items[i],
                    style: AidogType.numSm.copyWith(color: theme.c.fg),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                SmallButton(
                  key: ValueKey('sandbox-tag-del-$i'),
                  label: '×',
                  onTap: () => widget.onChanged([
                    for (var j = 0; j < widget.items.length; j++)
                      if (j != i) widget.items[j],
                  ]),
                ),
              ],
            ),
          ),
        Row(
          children: [
            Expanded(
              child: TextField(
                controller: _draft,
                style: AidogType.micro.copyWith(color: theme.c.fg),
                decoration: InputDecoration(isDense: true, hintText: widget.hint),
                onChanged: (_) => setState(() {}),
                onSubmitted: (_) => _add(),
              ),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              label: '+',
              onTap: _draft.text.trim().isEmpty ? null : _add,
            ),
          ],
        ),
      ],
    );
  }
}

/// 可增删的路径列表（带目录补全）。对应 React 的 `PathList`
/// （`SandboxSection.tsx:71`，它用的也是同一个 `PathInput`）。
class SandboxPathList extends StatefulWidget {
  const SandboxPathList({
    super.key,
    required this.items,
    required this.onChanged,
    this.hint,
    this.invoke = kernelInvoke,
    this.showPicker = true,
  });

  final List<String> items;
  final ValueChanged<List<String>> onChanged;
  final String? hint;
  final InvokeFn invoke;
  final bool showPicker;

  @override
  State<SandboxPathList> createState() => _SandboxPathListState();
}

class _SandboxPathListState extends State<SandboxPathList> {
  String? _draft;

  void _add() {
    final v = (_draft ?? '').trim();
    if (v.isEmpty || widget.items.contains(v)) {
      setState(() => _draft = null);
      return;
    }
    widget.onChanged([...widget.items, v]);
    setState(() => _draft = null);
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (var i = 0; i < widget.items.length; i++)
          Padding(
            padding: const EdgeInsets.only(bottom: 2),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    widget.items[i],
                    style: AidogType.numSm.copyWith(color: theme.c.fg),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                SmallButton(
                  key: ValueKey('sandbox-path-del-$i'),
                  label: '×',
                  onTap: () => widget.onChanged([
                    for (var j = 0; j < widget.items.length; j++)
                      if (j != i) widget.items[j],
                  ]),
                ),
              ],
            ),
          ),
        Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Expanded(
              // 每加一项就换一个 key：PathInputRow 的 controller 活在 State 里，
              // 不换 key 的话清空草稿不会反映到输入框上。
              child: PathInputRow(
                key: ValueKey('sandbox-path-draft-${widget.items.length}'),
                label: '',
                value: _draft,
                pathType: 'directory',
                hint: widget.hint ?? t.t('settings.editor.dirOrPathPh'),
                invoke: widget.invoke,
                showPicker: widget.showPicker,
                onChanged: (v) => setState(() => _draft = v),
              ),
            ),
            const SizedBox(width: AidogSpace.sxs),
            SmallButton(
              label: '+',
              onTap: (_draft ?? '').trim().isEmpty ? null : _add,
            ),
          ],
        ),
      ],
    );
  }
}

/// 沙箱编辑器本体。
class SandboxEditor extends StatelessWidget {
  const SandboxEditor({
    super.key,
    required this.sandbox,
    required this.onChanged,
    this.invoke = kernelInvoke,
    this.showPicker = true,
  });

  /// `settings.json` 的 `sandbox` 子树（缺省传空 map）。
  final Map<String, Object?> sandbox;

  /// 写回整棵子树；空树回传 `null`（= 删掉 `sandbox` 键）。
  final ValueChanged<Map<String, Object?>?> onChanged;

  final InvokeFn invoke;
  final bool showPicker;

  Map<String, Object?> get _fs => sandbox['filesystem'] is Map
      ? Map<String, Object?>.from(sandbox['filesystem'] as Map)
      : <String, Object?>{};

  Map<String, Object?> get _net => sandbox['network'] is Map
      ? Map<String, Object?>.from(sandbox['network'] as Map)
      : <String, Object?>{};

  bool get _enabled => sandbox['enabled'] == true;

  static List<String> _list(Map<String, Object?> m, String k) =>
      m[k] is List ? (m[k] as List).map((e) => '$e').toList() : const [];

  /// React `sync`（`SandboxSection.tsx:140`）的逐行镜像。
  void _sync(Map<String, Object?> patch) {
    final next = {...sandbox, ...patch};
    next.removeWhere((_, v) =>
        (v is List && v.isEmpty) || v == false || v == null);
    if (next['filesystem'] is Map) {
      final fso = Map<String, Object?>.from(next['filesystem'] as Map)
        ..removeWhere((_, v) => v is List && v.isEmpty);
      if (fso.isEmpty) {
        next.remove('filesystem');
      } else {
        next['filesystem'] = fso;
      }
    }
    if (next['network'] is Map) {
      final no = Map<String, Object?>.from(next['network'] as Map)
        ..removeWhere((_, v) => (v is List && v.isEmpty) || v == false || v == null);
      if (no.isEmpty) {
        next.remove('network');
      } else {
        next['network'] = no;
      }
    }
    onChanged(next.isEmpty ? null : next);
  }

  void _setFs(String key, List<String> arr) =>
      _sync({'filesystem': {..._fs, key: arr}});

  void _setNet(String key, List<String> arr) =>
      _sync({'network': {..._net, key: arr}});

  /// 端口：非法值（非数字 / 越界）直接不写，保持原值 —— React 的 `setNetPort`
  /// 在 `isNaN || <0 || >65535` 时直接 `return`，不是写回 undefined。
  void _setPort(String key, String raw) {
    if (raw.isEmpty) {
      _sync({'network': {..._net, key: null}});
      return;
    }
    final port = int.tryParse(raw);
    if (port == null || port < 0 || port > 65535) return;
    _sync({'network': {..._net, key: port}});
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Column(
      key: const ValueKey('field-sandbox'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        SwitchRow(
          key: const ValueKey('sandbox-enabled'),
          label: t.t('settings.sandbox.enable'),
          description: t.t('settings.sandbox.enableDesc'),
          value: _enabled,
          onChanged: (v) => _sync({'enabled': v}),
        ),
        if (_enabled)
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
            child: Text(
              '● ${t.t('settings.sandbox.enabled')}',
              style: AidogType.micro.copyWith(color: theme.c.ok),
            ),
          )
        else
          Padding(
            padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
            child: Text(
              t.t('settings.sandbox.disabledHint'),
              style: AidogType.micro.copyWith(color: theme.c.fg3),
            ),
          ),
        if (_enabled) ...[
          // ── 文件系统隔离 ──
          TileMeta(t.t('settings.sandbox.fsIsolation')),
          Text(
            t.t('settings.sandbox.fsIsolationDesc'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.sxs),
          _pathField(t, 'allowWrite', 'settings.sandbox.allowWrite', 'settings.sandbox.allowWritePh'),
          _pathField(t, 'denyWrite', 'settings.sandbox.denyWrite', 'settings.sandbox.denyWritePh'),
          _pathField(t, 'allowRead', 'settings.sandbox.allowRead', 'settings.sandbox.allowReadPh'),
          _pathField(t, 'denyRead', 'settings.sandbox.denyRead', 'settings.sandbox.denyReadPh'),

          // ── 网络隔离 ──
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('settings.sandbox.netIsolation')),
          Text(
            t.t('settings.sandbox.netIsolationDesc'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.sxs),
          _tagField(t, 'allowedDomains', 'settings.sandbox.allowedDomains',
              'settings.sandbox.allowedDomainsPh'),
          _tagField(t, 'deniedDomains', 'settings.sandbox.deniedDomains',
              'settings.sandbox.deniedDomainsPh'),
          TextRow(
            key: const ValueKey('sandbox-http-proxy'),
            label: t.t('settings.sandbox.httpProxy'),
            hint: t.t('settings.sandbox.port'),
            value: '${_net['httpProxyPort'] ?? ''}',
            onSubmitted: (v) => _setPort('httpProxyPort', v.trim()),
          ),
          TextRow(
            key: const ValueKey('sandbox-socks-proxy'),
            label: t.t('settings.sandbox.socksProxy'),
            hint: t.t('settings.sandbox.port'),
            value: '${_net['socksProxyPort'] ?? ''}',
            onSubmitted: (v) => _setPort('socksProxyPort', v.trim()),
          ),

          // ── 安全与策略 ──
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('settings.sandbox.safety')),
          SwitchRow(
            key: const ValueKey('sandbox-fail-if-unavailable'),
            label: t.t('settings.sandbox.failIfUnavailable'),
            description: t.t('settings.sandbox.failIfUnavailableDesc'),
            value: sandbox['failIfUnavailable'] == true,
            onChanged: (v) => _sync({'failIfUnavailable': v}),
          ),
          SwitchRow(
            key: const ValueKey('sandbox-no-escape'),
            label: t.t('settings.sandbox.noEscape'),
            description: t.t('settings.sandbox.noEscapeDesc'),
            // 反向开关：开 = 禁止逃逸 = allowUnsandboxedCommands: false。
            value: sandbox['allowUnsandboxedCommands'] == false,
            onChanged: (v) => _sync({'allowUnsandboxedCommands': !v}),
          ),
          SwitchRow(
            key: const ValueKey('sandbox-lock-domains'),
            label: t.t('settings.sandbox.lockDomains'),
            description: t.t('settings.sandbox.lockDomainsDesc'),
            value: _net['allowManagedDomainsOnly'] == true,
            onChanged: (v) => _sync({'network': {..._net, 'allowManagedDomainsOnly': v}}),
          ),
          SwitchRow(
            key: const ValueKey('sandbox-lock-read-paths'),
            label: t.t('settings.sandbox.lockReadPaths'),
            description: t.t('settings.sandbox.lockReadPathsDesc'),
            value: sandbox['allowManagedReadPathsOnly'] == true,
            onChanged: (v) => _sync({'allowManagedReadPathsOnly': v}),
          ),
          SwitchRow(
            key: const ValueKey('sandbox-weak-net'),
            label: t.t('settings.sandbox.weakNet'),
            description: t.t('settings.sandbox.weakNetDesc'),
            value: sandbox['enableWeakerNetworkIsolation'] == true,
            onChanged: (v) => _sync({'enableWeakerNetworkIsolation': v}),
          ),
          SwitchRow(
            key: const ValueKey('sandbox-weak-nested'),
            label: t.t('settings.sandbox.weakNested'),
            description: t.t('settings.sandbox.weakNestedDesc'),
            value: sandbox['enableWeakerNestedSandbox'] == true,
            onChanged: (v) => _sync({'enableWeakerNestedSandbox': v}),
          ),
          SwitchRow(
            key: const ValueKey('sandbox-unix-sockets'),
            label: t.t('settings.sandbox.unixSockets'),
            description: t.t('settings.sandbox.unixSocketsDesc'),
            value: sandbox['allowUnixSockets'] == true,
            onChanged: (v) => _sync({'allowUnixSockets': v}),
          ),

          // ── 排除命令 ──
          const SizedBox(height: AidogSpace.ssm),
          TileMeta(t.t('settings.sandbox.excludedCommands')),
          Text(
            t.t('settings.sandbox.excludedCommandsDesc'),
            style: AidogType.micro.copyWith(color: theme.c.fg3),
          ),
          const SizedBox(height: AidogSpace.sxs),
          SandboxTagList(
            key: const ValueKey('sandbox-excluded-commands'),
            items: _list(sandbox, 'excludedCommands'),
            hint: t.t('settings.sandbox.excludedCommandsPh'),
            onChanged: (v) => _sync({'excludedCommands': v}),
          ),
        ],
      ],
    );
  }

  Widget _pathField(I18nController t, String key, String labelKey, String hintKey) => Padding(
        padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            TileMeta(t.t(labelKey)),
            SandboxPathList(
              key: ValueKey('sandbox-fs-$key'),
              items: _list(_fs, key),
              hint: t.t(hintKey),
              invoke: invoke,
              showPicker: showPicker,
              onChanged: (v) => _setFs(key, v),
            ),
          ],
        ),
      );

  Widget _tagField(I18nController t, String key, String labelKey, String hintKey) => Padding(
        padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            TileMeta(t.t(labelKey)),
            SandboxTagList(
              key: ValueKey('sandbox-net-$key'),
              items: _list(_net, key),
              hint: t.t(hintKey),
              onChanged: (v) => _setNet(key, v),
            ),
          ],
        ),
      );
}
