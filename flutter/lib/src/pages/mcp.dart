/// MCP 页界面（票 I09），对应 `src/pages/Mcp.tsx` 的主列表 + 五个弹窗。
///
/// 五个弹窗（扫描导入 / 粘贴导入 / 删除确认 / 编辑 / 分享）与票 I07 同一做法：
/// 页面 state 的一部分，不是 route，widget 测试直接断言。色值一律走主题。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'invoke.dart';
import 'mcp_logic.dart';
import 'ui_bits.dart';

class McpPage extends StatefulWidget {
  const McpPage({super.key, this.invoke = kernelInvoke});

  final InvokeFn invoke;

  @override
  State<McpPage> createState() => _McpPageState();
}

class _McpPageState extends State<McpPage> {
  late final McpController _c;

  bool _built = false;

  /// 控制器的 t 必须来自 context（全局 `i18n` 在 widget 测试里没 init 过）。
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_built) return;
    _built = true;
    _c = McpController(
      invoke: widget.invoke,
      t: AidogI18n.of(context).t,
      onChanged: () {
        if (mounted) setState(() {});
      },
    );
    _c.refresh();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        PageHead(
          title: t.t('mcp.title'),
          subtitle: ltr('${_c.servers.length}'),
          trailing: Wrap(
            spacing: AidogSpace.ssm,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              SmallButton(label: t.t('mcp.add'), onTap: _c.openAdd),
              SmallButton(label: t.t('mcp.scanImport'), onTap: _c.openScan),
              SmallButton(
                label: t.t('mcp.pasteImport'),
                onTap: () => _c.setPasteOpen(true),
              ),
              SmallButton(
                label: t.t('mcp.resync'),
                onTap: _c.busyKey == null ? _c.resync : null,
              ),
            ],
          ),
        ),
        const Padding(
          padding: EdgeInsets.only(bottom: AidogSpace.ssm),
          child: PiUnsupportedNote(
            reasonKey: 'pi.unsupportedMcp',
            reasonFallback:
                'pi 刻意不内置 MCP，能力靠 extension 直接写 TypeScript 提供，没有可写入的 MCP 配置文件。',
          ),
        ),
        if (_c.loading)
          CenteredNote(text: t.t('status.loading'))
        else if (_c.servers.isEmpty)
          CenteredNote(text: t.t('mcp.empty'))
        else
          Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final s in _c.servers)
                Padding(
                  padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                  child: _McpRow(
                    server: s,
                    busy: _c.busyKey != null,
                    onToggleAgent: (a) => _c.toggle(s, a),
                    onEdit: () => _c.openEdit(s),
                    onShare: () => _c.share(s),
                    onDelete: () => _c.askDelete(s),
                  ),
                ),
            ],
          ),
        if (_c.scanOpen) _scanCard(t),
        if (_c.pasteOpen) _pasteCard(t),
        if (_c.editOpen) _editCard(t),
        if (_c.deleteTarget != null)
          ConfirmCard(
            title: t.t('mcp.deleteTitle'),
            body: t.t('mcp.deleteConfirm', {'name': _c.deleteTarget!.name}),
            confirmLabel: t.t('action.delete'),
            onCancel: _c.cancelDelete,
            onConfirm: _c.delete,
          ),
        if (_c.shareData != null) _shareCard(t),
        if (_c.message != null)
          ToastBar(text: _c.message!.text, ok: _c.message!.ok),
      ],
    );
  }

  // React 是普通 `Dialog`（`McpModals.tsx:54`，maxWidth 560）：导入中不许关。
  Widget _scanCard(I18nController t) => AidogModal(
    maxWidth: 560,
    onBarrierTap: _c.importing ? null : _c.closeScan,
    child: Tile(
      title: t.t('mcp.scanTitle'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          if (_c.scanning)
            Text(
              t.t('status.loading'),
              style: AidogType.micro.copyWith(
                color: AidogTheme.of(context).c.fg3,
              ),
            )
          else if (_c.scanItems.isEmpty)
            Text(
              t.t('mcp.scanEmpty'),
              style: AidogType.micro.copyWith(
                color: AidogTheme.of(context).c.fg3,
              ),
            )
          else
            for (final it in _c.scanItems)
              Row(
                children: [
                  Checkbox(
                    value: _c.selected.contains(it.name),
                    onChanged: _c.importing
                        ? null
                        : (_) => _c.toggleSelect(it.name),
                  ),
                  Expanded(
                    child: Text(
                      '${it.name} · ${it.transport}'
                      '${it.alreadyImported ? ' · ${t.t('mcp.alreadyImported')}' : ''}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.micro.copyWith(
                        color: AidogTheme.of(context).c.fg2,
                      ),
                    ),
                  ),
                ],
              ),
          const SizedBox(height: AidogSpace.ssm),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SmallButton(
                label: t.t('action.cancel'),
                onTap: _c.importing ? null : _c.closeScan,
              ),
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(
                label: _c.importing
                    ? t.t('mcp.importing')
                    : t.t('mcp.import', {'count': _c.selected.length}),
                // 一条都没勾就没得导（React 里 handleImport 直接 return）。
                onTap: _c.importing || _c.selected.isEmpty
                    ? null
                    : _c.importSelected,
              ),
            ],
          ),
        ],
      ),
    ),
  );

  // React 是普通 `Dialog`（`McpModals.tsx:174`，maxWidth 560）：导入中不许关。
  Widget _pasteCard(I18nController t) => AidogModal(
    maxWidth: 560,
    onBarrierTap: _c.pasteBusy ? null : () => _c.setPasteOpen(false),
    child: Tile(
      title: t.t('mcp.pasteImport'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            key: const Key('mcp-paste'),
            maxLines: 4,
            decoration: InputDecoration(
              isDense: true,
              hintText: t.t('mcp.pasteHint'),
            ),
            style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg),
            onChanged: _c.setPasteText,
          ),
          const SizedBox(height: AidogSpace.ssm),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SmallButton(
                label: t.t('action.cancel'),
                onTap: _c.pasteBusy ? null : () => _c.setPasteOpen(false),
              ),
              const SizedBox(width: AidogSpace.ssm),
              SmallButton(
                label: _c.pasteBusy
                    ? t.t('status.loading')
                    : t.t('action.confirm'),
                onTap: _c.pasteBusy || _c.pasteText.trim().isEmpty
                    ? null
                    : _c.pasteImport,
              ),
            ],
          ),
        ],
      ),
    ),
  );

  Widget _kvEditor(I18nController t, String label, List<KvRow> rows) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: [
      Row(
        children: [
          Expanded(child: TileMeta(label)),
          SmallButton(
            label: t.t('mcp.addRow'),
            onTap: () => setState(() => rows.add(KvRow('', ''))),
          ),
        ],
      ),
      for (var i = 0; i < rows.length; i++)
        Padding(
          padding: const EdgeInsets.only(top: AidogSpace.sxs),
          child: Row(
            children: [
              Expanded(
                child: TextField(
                  controller: TextEditingController(text: rows[i].k),
                  decoration: const InputDecoration(isDense: true),
                  style: AidogType.micro.copyWith(
                    color: AidogTheme.of(context).c.fg,
                  ),
                  onChanged: (v) => rows[i].k = v,
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              Expanded(
                child: TextField(
                  controller: TextEditingController(text: rows[i].v),
                  decoration: const InputDecoration(isDense: true),
                  style: AidogType.micro.copyWith(
                    color: AidogTheme.of(context).c.fg,
                  ),
                  onChanged: (v) => rows[i].v = v,
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              SmallButton(
                label: t.t('action.delete'),
                danger: true,
                onTap: () => setState(() => rows.removeAt(i)),
              ),
            ],
          ),
        ),
    ],
  );

  Widget _editCard(I18nController t) {
    final f = _c.editForm;
    final theme = AidogTheme.of(context);
    // React 是普通 `Dialog`（`McpModals.tsx:241`，maxWidth 560），点遮罩可关。
    return AidogModal(
      maxWidth: 560,
      onBarrierTap: _c.closeEdit,
      child: Tile(
        title: _c.editTarget == null ? t.t('mcp.add') : t.t('mcp.edit'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              key: const Key('mcp-name'),
              controller: TextEditingController(text: f.name)
                ..selection = TextSelection.collapsed(offset: f.name.length),
              decoration: InputDecoration(
                isDense: true,
                hintText: t.t('mcp.field.name'),
              ),
              style: AidogType.micro.copyWith(color: theme.c.fg),
              onChanged: (v) => f.name = v,
            ),
            const SizedBox(height: AidogSpace.ssm),
            Wrap(
              spacing: AidogSpace.sxs,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                TileMeta(t.t('mcp.field.transport')),
                for (final tr in const ['stdio', 'http', 'sse'])
                  SmallButton(
                    label: tr,
                    active: f.transport == tr,
                    onTap: () => setState(() => f.transport = tr),
                  ),
              ],
            ),
            const SizedBox(height: AidogSpace.ssm),
            // stdio 用 command + args；http / sse 用 url + headers。
            if (f.transport == 'stdio') ...[
              TextField(
                key: const Key('mcp-command'),
                controller: TextEditingController(text: f.command),
                decoration: InputDecoration(
                  isDense: true,
                  hintText: t.t('mcp.field.command'),
                ),
                style: AidogType.micro.copyWith(color: theme.c.fg),
                onChanged: (v) => f.command = v,
              ),
              const SizedBox(height: AidogSpace.sxs),
              TextField(
                key: const Key('mcp-args'),
                maxLines: 3,
                controller: TextEditingController(text: f.argsText),
                decoration: InputDecoration(
                  isDense: true,
                  hintText: t.t('mcp.field.args'),
                ),
                style: AidogType.micro.copyWith(color: theme.c.fg),
                onChanged: (v) => f.argsText = v,
              ),
              const SizedBox(height: AidogSpace.ssm),
              _kvEditor(t, t.t('mcp.field.env'), f.envRows),
            ] else ...[
              TextField(
                key: const Key('mcp-url'),
                controller: TextEditingController(text: f.url),
                decoration: InputDecoration(
                  isDense: true,
                  hintText: t.t('mcp.field.url'),
                ),
                style: AidogType.micro.copyWith(color: theme.c.fg),
                onChanged: (v) => f.url = v,
              ),
              const SizedBox(height: AidogSpace.ssm),
              _kvEditor(t, t.t('mcp.field.headers'), f.headersRows),
            ],
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(label: t.t('action.cancel'), onTap: _c.closeEdit),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: t.t('action.save'),
                  onTap: _c.busyKey != null ? null : _c.saveEdit,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _shareCard(I18nController t) {
    final data = _c.shareData!;
    final text = data.share.toString();
    // React 走同一个 `ShareModal`（maxWidth 560），点遮罩可关。
    return AidogModal(
      maxWidth: 560,
      onBarrierTap: _c.closeShare,
      child: Tile(
        title: '${t.t('mcp.share.title')} · ${data.name}',
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            SelectableText(
              text,
              style: AidogType.micro.copyWith(
                color: AidogTheme.of(context).c.fg2,
              ),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('mcp.share.copyUrl'),
                  onTap: () => native.writeText(text),
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(label: t.t('action.close'), onTap: _c.closeShare),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _McpRow extends StatelessWidget {
  const _McpRow({
    required this.server,
    required this.busy,
    required this.onToggleAgent,
    required this.onEdit,
    required this.onShare,
    required this.onDelete,
  });

  final McpServerInfo server;
  final bool busy;
  final void Function(String agent) onToggleAgent;
  final VoidCallback onEdit;
  final VoidCallback onShare;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Tile(
      child: Row(
        children: [
          Expanded(
            flex: 3,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  server.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AidogType.body.copyWith(color: theme.c.fg),
                ),
                Text(
                  server.summary,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AidogType.micro.copyWith(color: theme.c.fg3),
                ),
              ],
            ),
          ),
          Expanded(
            child: Text(
              server.transport,
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
          ),
          Wrap(
            spacing: AidogSpace.sxs,
            children: [
              // codex 只支持 stdio：不支持的组合点下去会得到一条错误提示（不是静默）。
              for (final a in kMcpAgents)
                SmallButton(
                  label: t.t('mcp.agent.$a'),
                  active: server.enabledAgents.contains(a),
                  onTap: busy ? null : () => onToggleAgent(a),
                ),
              SmallButton(label: t.t('action.edit'), onTap: onEdit),
              SmallButton(label: t.t('mcp.share'), onTap: onShare),
              SmallButton(
                label: t.t('action.delete'),
                danger: true,
                onTap: onDelete,
              ),
            ],
          ),
        ],
      ),
    );
  }
}
