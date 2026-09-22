/// MCP 页的逻辑层（票 I09），对应 `src/pages/Mcp.tsx` + `Mcp/useMcpData.ts`
/// + `Mcp/constants.ts`。
///
/// **字段名是 camelCase**：MCP 这一族后端是 `#[serde(rename_all = "camelCase")]`
/// （`manual.ts:63` 那条注释写死了这是跨层 snake_case 的唯一例外），和 Skills 那族
/// 的 snake_case 不同，抄的时候别串行。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart' show VoidCallback;

import 'invoke.dart';
import 'skills_logic.dart' show TrFn;

/// `Mcp/constants.ts:5`。与 Skills 的 `claude` / `codex` **不是同一套 slug**。
const List<String> kMcpAgents = ['claude-code', 'codex'];

/// `Mcp/constants.ts:13::agentSupported`：codex 仅 stdio；claude-code 全支持。
bool mcpAgentSupported(String transport, String agent) {
  if (agent == 'codex') return transport == 'stdio';
  return true;
}

/// `Mcp/constants.ts:34::summaryOf`：stdio → command + 首参；http/sse → url（空给 `—`）。
String mcpSummaryOf({
  required String transport,
  required String command,
  required List<String> args,
  required String url,
}) {
  if (transport == 'stdio') {
    final first = args.isNotEmpty ? args.first : '';
    return [command, first].where((s) => s.isNotEmpty).join(' ');
  }
  return url.isNotEmpty ? url : '—';
}

Map<String, String> _strMap(Object? raw) => {
  for (final e in ((raw as Map?) ?? const {}).entries) '${e.key}': '${e.value}',
};

List<String> _strList(Object? raw) => [
  for (final e in ((raw as List<Object?>?) ?? const [])) '$e',
];

/// `manual.ts:74::McpServerInfo`。env / headers 已被后端脱敏成 `***`。
class McpServerInfo {
  const McpServerInfo({
    required this.id,
    required this.name,
    required this.transport,
    required this.command,
    required this.args,
    required this.env,
    required this.url,
    required this.headers,
    required this.enabledAgents,
    this.createdAt = 0,
    this.updatedAt = 0,
  });

  final int id;
  final String name;
  final String transport;
  final String command;
  final List<String> args;
  final Map<String, String> env;
  final String url;
  final Map<String, String> headers;
  final List<String> enabledAgents;
  final int createdAt;
  final int updatedAt;

  static McpServerInfo fromJson(Map<String, Object?> j) => McpServerInfo(
    id: (j['id'] as num?)?.toInt() ?? 0,
    name: (j['name'] as String?) ?? '',
    transport: (j['transport'] as String?) ?? 'stdio',
    command: (j['command'] as String?) ?? '',
    args: _strList(j['args']),
    env: _strMap(j['env']),
    url: (j['url'] as String?) ?? '',
    headers: _strMap(j['headers']),
    enabledAgents: _strList(j['enabledAgents']),
    createdAt: (j['createdAt'] as num?)?.toInt() ?? 0,
    updatedAt: (j['updatedAt'] as num?)?.toInt() ?? 0,
  );

  McpServerInfo withAgents(List<String> agents) => McpServerInfo(
    id: id,
    name: name,
    transport: transport,
    command: command,
    args: args,
    env: env,
    url: url,
    headers: headers,
    enabledAgents: agents,
    createdAt: createdAt,
    updatedAt: updatedAt,
  );

  String get summary => mcpSummaryOf(
    transport: transport,
    command: command,
    args: args,
    url: url,
  );
}

/// `manual.ts:91::McpScanItem`。
class McpScanItem {
  const McpScanItem({
    required this.name,
    required this.transport,
    required this.command,
    required this.args,
    required this.env,
    required this.url,
    required this.headers,
    required this.foundInAgents,
    required this.alreadyImported,
  });

  final String name;
  final String transport;
  final String command;
  final List<String> args;
  final Map<String, String> env;
  final String url;
  final Map<String, String> headers;
  final List<String> foundInAgents;
  final bool alreadyImported;

  static McpScanItem fromJson(Map<String, Object?> j) => McpScanItem(
    name: (j['name'] as String?) ?? '',
    transport: (j['transport'] as String?) ?? 'stdio',
    command: (j['command'] as String?) ?? '',
    args: _strList(j['args']),
    env: _strMap(j['env']),
    url: (j['url'] as String?) ?? '',
    headers: _strMap(j['headers']),
    foundInAgents: _strList(j['foundInAgents']),
    alreadyImported: j['alreadyImported'] == true,
  );

  /// `useMcpData.ts:150` 的导入载荷：来源 agent 取**首个发现 agent**，
  /// 空数组回落 `claude-code`（`?? "claude-code"`）。
  Map<String, Object?> toImportPayload() => {
    'name': name,
    'transport': transport,
    'command': command,
    'args': args,
    'env': env,
    'url': url,
    'headers': headers,
    'sourceAgent': foundInAgents.isNotEmpty
        ? foundInAgents.first
        : 'claude-code',
  };
}

/// `manual.ts:115::McpImportReport`。
class McpImportReport {
  const McpImportReport({required this.imported, required this.skipped});

  final List<String> imported;
  final List<String> skipped;

  static McpImportReport fromJson(Map<String, Object?> j) => McpImportReport(
    imported: _strList(j['imported']),
    skipped: _strList(j['skipped']),
  );
}

/// 编辑表单的一行键值（env / headers 都用它）。
class KvRow {
  KvRow(this.k, this.v);

  String k;
  String v;
}

/// 编辑 / 新增表单态（`useMcpData.ts:46` 的 `editForm`）。
class McpEditForm {
  McpEditForm({
    this.name = '',
    this.transport = 'stdio',
    this.command = '',
    this.argsText = '',
    List<KvRow>? envRows,
    this.url = '',
    List<KvRow>? headersRows,
  }) : envRows = envRows ?? [],
       headersRows = headersRows ?? [];

  String name;
  String transport;
  String command;
  String argsText;
  List<KvRow> envRows;
  String url;
  List<KvRow> headersRows;

  /// `useMcpData.ts:331::openEdit`：args 用换行连接，env / headers 铺成行。
  static McpEditForm fromServer(McpServerInfo s) => McpEditForm(
    name: s.name,
    transport: s.transport,
    command: s.command,
    argsText: s.args.join('\n'),
    envRows: [for (final e in s.env.entries) KvRow(e.key, e.value)],
    url: s.url,
    headersRows: [for (final e in s.headers.entries) KvRow(e.key, e.value)],
  );

  /// `useMcpData.ts:372` 的 payload：name trim；args 按行 trim 后剔空；
  /// env / headers 只留 key 非空的行（**value 不 trim**，照搬）。
  Map<String, Object?> toPayload() => {
    'name': name.trim(),
    'transport': transport,
    'command': command,
    'args': [
      for (final a in argsText.split('\n'))
        if (a.trim().isNotEmpty) a.trim(),
    ],
    'env': {
      for (final r in envRows)
        if (r.k.trim().isNotEmpty) r.k.trim(): r.v,
    },
    'url': url,
    'headers': {
      for (final r in headersRows)
        if (r.k.trim().isNotEmpty) r.k.trim(): r.v,
    },
  };
}

final RegExp _mcpBase64Shape = RegExp(r'^[A-Za-z0-9+/=\s]+$');

/// `useMcpData.ts:192` 的粘贴文本归一化：形如 base64 且解码后是**合法 JSON** 才用解码结果，
/// 否则原样。后端 `mcp_import_json` 走 serde_json，所以这里只认 JSON。
String normalizeMcpPaste(String raw) {
  final json = raw.trim();
  if (_mcpBase64Shape.hasMatch(json) && json.length > 16) {
    try {
      final decoded = utf8.decode(
        base64.decode(json.replaceAll(RegExp(r'\s'), '')),
      );
      jsonDecode(decoded); // 验证是合法 JSON，不合法就保持原文本
      return decoded;
    } catch (_) {
      // 非 base64 或解码后非 JSON。
    }
  }
  return json;
}

/// `useMcpData.ts` 的 Dart 版。
class McpController {
  McpController({
    required this.invoke,
    required this.t,
    required this.onChanged,
  });

  final InvokeFn invoke;
  final TrFn t;
  final VoidCallback onChanged;

  List<McpServerInfo> servers = const [];
  bool loading = true;
  String? busyKey;

  /// `{kind:'ok'|'err', text}`。
  ({bool ok, String text})? message;

  bool scanOpen = false;
  List<McpScanItem> scanItems = const [];
  bool scanning = false;
  final Set<String> selected = <String>{};
  bool importing = false;

  bool pasteOpen = false;
  String pasteText = '';
  bool pasteBusy = false;

  McpServerInfo? deleteTarget;

  McpServerInfo? editTarget;
  bool editOpen = false;
  McpEditForm editForm = McpEditForm();

  /// 分享弹窗：后端导出的可分享配置 + server 名。
  ({Map<String, Object?> share, String name})? shareData;

  void _ok(String text) {
    message = (ok: true, text: text);
    onChanged();
  }

  void _err(Object e) {
    message = (ok: false, text: '$e');
    onChanged();
  }

  void clearMessage() {
    message = null;
    onChanged();
  }

  /// 给分享面板用的 toast 入口（`McpModals.tsx:339` 的 `onToast`）。
  void showToast(String text, {required bool ok}) {
    message = (ok: ok, text: text);
    onChanged();
  }

  /// `useMcpData.ts:64::refresh`。**失败也要把 loading 落下来**（finally）。
  Future<void> refresh() async {
    try {
      final raw = await invoke('mcp_list');
      servers = [
        for (final s in (raw as List<Object?>))
          McpServerInfo.fromJson((s as Map).cast<String, Object?>()),
      ];
    } catch (e) {
      _err(e);
    } finally {
      loading = false;
      onChanged();
    }
  }

  /// `useMcpData.ts:80::handleToggle`。
  /// 禁用永远允许；启用要先过 [mcpAgentSupported]（codex + http/sse 直接报错不发命令）。
  Future<void> toggle(McpServerInfo srv, String agent) async {
    if (busyKey != null) return;
    final enabled = srv.enabledAgents.contains(agent);
    if (!enabled && !mcpAgentSupported(srv.transport, agent)) {
      message = (
        ok: false,
        text: t('mcp.unsupportedTransport', {
          'transport': srv.transport,
          'agent': t('mcp.agent.$agent'),
        }),
      );
      onChanged();
      return;
    }
    busyKey = '${srv.name}::$agent';
    message = null;
    final prev = servers;
    servers = [
      for (final s in servers)
        if (s.name == srv.name)
          s.withAgents(
            enabled
                ? [
                    for (final a in s.enabledAgents)
                      if (a != agent) a,
                  ]
                : [...s.enabledAgents, agent],
          )
        else
          s,
    ];
    onChanged();
    try {
      await invoke('mcp_set_agent', {
        'name': srv.name,
        'agent': agent,
        'enabled': !enabled,
      });
      _ok(t(enabled ? 'mcp.disabled' : 'mcp.enabled'));
    } catch (e) {
      servers = prev; // 回滚
      _err(e);
    } finally {
      busyKey = null;
      onChanged();
    }
  }

  /// `useMcpData.ts:126::openScan`。**默认预选所有未导入项**；扫描失败把弹窗关掉。
  Future<void> openScan() async {
    scanOpen = true;
    scanning = true;
    selected.clear();
    onChanged();
    try {
      final raw = await invoke('mcp_scan');
      scanItems = [
        for (final i in (raw as List<Object?>))
          McpScanItem.fromJson((i as Map).cast<String, Object?>()),
      ];
      selected.addAll(
        scanItems.where((i) => !i.alreadyImported).map((i) => i.name),
      );
    } catch (e) {
      _err(e);
      scanOpen = false;
    } finally {
      scanning = false;
      onChanged();
    }
  }

  void closeScan() {
    scanOpen = false;
    onChanged();
  }

  void toggleSelect(String name) {
    if (!selected.remove(name)) selected.add(name);
    onChanged();
  }

  /// 全选 / 反选（`McpModals.tsx:63-81`）：已导入的不参与 —— 它们本来就勾不动。
  /// 当前「可选的都选上了」就清空，否则全选。
  void toggleSelectAll() {
    final selectable = [
      for (final it in scanItems)
        if (!it.alreadyImported) it.name,
    ];
    final allOn =
        selectable.isNotEmpty && selectable.every(selected.contains);
    selected.clear();
    if (!allOn) selected.addAll(selectable);
    onChanged();
  }


  String _importText(McpImportReport report) {
    final skipped = report.skipped.length;
    return skipped > 0
        ? t('mcp.importPartial', {
            'ok': report.imported.length,
            'skip': skipped,
          })
        : t('mcp.imported', {'count': report.imported.length});
  }

  /// `useMcpData.ts:143::handleImport`。跳过非空 = 按 err 色出（React 的 `kind`）。
  Future<void> importSelected() async {
    if (selected.isEmpty) return;
    importing = true;
    message = null;
    onChanged();
    try {
      final items = [
        for (final it in scanItems)
          if (selected.contains(it.name)) it.toImportPayload(),
      ];
      final raw = await invoke('mcp_import', {'items': items});
      final report = McpImportReport.fromJson(
        (raw as Map).cast<String, Object?>(),
      );
      await refresh();
      scanOpen = false;
      message = (ok: report.skipped.isEmpty, text: _importText(report));
    } catch (e) {
      _err(e);
    } finally {
      importing = false;
      onChanged();
    }
  }

  void setPasteOpen(bool open) {
    pasteOpen = open;
    onChanged();
  }

  void setPasteText(String s) {
    pasteText = s;
    onChanged();
  }

  /// `useMcpData.ts:187::handlePasteImport`。空白文本不发命令。
  Future<void> pasteImport() async {
    if (pasteText.trim().isEmpty) return;
    pasteBusy = true;
    message = null;
    onChanged();
    try {
      final raw = await invoke('mcp_import_json', {
        'json': normalizeMcpPaste(pasteText),
      });
      final report = McpImportReport.fromJson(
        (raw as Map).cast<String, Object?>(),
      );
      await refresh();
      pasteOpen = false;
      pasteText = '';
      message = (ok: report.skipped.isEmpty, text: _importText(report));
    } catch (e) {
      _err(e);
    } finally {
      pasteBusy = false;
      onChanged();
    }
  }

  /// `useMcpData.ts:286::openDeepLinkImport`。`aidog://mcp/import?data=<base64>`。
  /// **这条路径不做 JSON 合法性预检**（React 直接 atob 扔给后端），照搬。
  Future<void> openDeepLinkImport(String data) async {
    if (data.isEmpty) return;
    message = null;
    onChanged();
    try {
      final json = utf8.decode(
        base64.decode(data.replaceAll(RegExp(r'\s'), '')),
      );
      final raw = await invoke('mcp_import_json', {'json': json});
      final report = McpImportReport.fromJson(
        (raw as Map).cast<String, Object?>(),
      );
      await refresh();
      message = (ok: report.skipped.isEmpty, text: _importText(report));
    } catch (e) {
      _err(e);
    }
  }

  void askDelete(McpServerInfo s) {
    deleteTarget = s;
    onChanged();
  }

  void cancelDelete() {
    deleteTarget = null;
    onChanged();
  }

  /// `useMcpData.ts:232::handleDelete`。成功就地移行，不整表重拉。
  Future<void> delete() async {
    final target = deleteTarget;
    if (target == null) return;
    busyKey = 'del::${target.name}';
    onChanged();
    try {
      await invoke('mcp_delete', {'name': target.name});
      servers = [
        for (final s in servers)
          if (s.name != target.name) s,
      ];
      _ok(t('mcp.deleted'));
    } catch (e) {
      _err(e);
    } finally {
      deleteTarget = null;
      busyKey = null;
      onChanged();
    }
  }

  /// `useMcpData.ts:248::handleResync`。
  Future<void> resync() async {
    if (busyKey != null) return;
    busyKey = 'resync';
    message = null;
    onChanged();
    try {
      final n = await invoke('mcp_resync');
      _ok(t('mcp.resyncDone', {'n': n}));
    } catch (e) {
      _err(e);
    } finally {
      busyKey = null;
      onChanged();
    }
  }

  /// `useMcpData.ts:272::handleShare`。
  Future<void> share(McpServerInfo srv) async {
    message = null;
    onChanged();
    try {
      final raw = await invoke('mcp_share_export', {'name': srv.name});
      shareData = (share: (raw as Map).cast<String, Object?>(), name: srv.name);
      onChanged();
    } catch (e) {
      _err(e);
    }
  }

  void closeShare() {
    shareData = null;
    onChanged();
  }

  void openEdit(McpServerInfo srv) {
    editTarget = srv;
    editForm = McpEditForm.fromServer(srv);
    message = null;
    editOpen = true;
    onChanged();
  }

  /// `useMcpData.ts:347::openAdd`：空表单，`editTarget = null` 即「新增」。
  void openAdd() {
    editTarget = null;
    editForm = McpEditForm();
    message = null;
    editOpen = true;
    onChanged();
  }

  void closeEdit() {
    editOpen = false;
    editTarget = null;
    onChanged();
  }

  /// `useMcpData.ts:362::handleEditSave`。name 空（trim 后）→ 只报错不发命令。
  Future<void> saveEdit() async {
    if (!editOpen) return;
    if (editForm.name.trim().isEmpty) {
      message = (ok: false, text: t('mcp.nameRequired'));
      onChanged();
      return;
    }
    final isAdd = editTarget == null;
    busyKey = isAdd ? 'add::' : 'edit::${editTarget!.name}';
    message = null;
    onChanged();
    final payload = editForm.toPayload();
    try {
      if (isAdd) {
        await invoke('mcp_add', {'payload': payload});
      } else {
        await invoke('mcp_update', {
          'oldName': editTarget!.name,
          'payload': payload,
        });
      }
      await refresh();
      editTarget = null;
      editOpen = false;
      _ok(t('mcp.saved'));
    } catch (e) {
      _err(e);
    } finally {
      busyKey = null;
      onChanged();
    }
  }
}
