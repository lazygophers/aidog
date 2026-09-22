/// MCP 页界面（票 I09），对应 `src/pages/Mcp.tsx` 的主列表 + 五个弹窗。
///
/// 五个弹窗（扫描导入 / 粘贴导入 / 删除确认 / 编辑 / 分享）与票 I07 同一做法：
/// 页面 state 的一部分，不是 route，widget 测试直接断言。色值一律走主题。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'invoke.dart';
import 'mcp_logic.dart';
import 'platform_logo.dart' show AgentIconButton;
import 'platform_card_bits.dart' show MiniBadge;
import 'share_panel.dart';
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
          // 全选 / 反选（`McpModals.tsx:63-81`）：十几条一条条点太慢。
          // 已导入的不参与（它们本来就勾不动）。扫描中 / 导入中禁用。
          Align(
            alignment: AlignmentDirectional.centerEnd,
            child: SmallButton(
              label: t.t('mcp.toggleAll'),
              onTap: (_c.scanning || _c.importing || _c.scanItems.isEmpty)
                  ? null
                  : _c.toggleSelectAll,
            ),
          ),
          const SizedBox(height: AidogSpace.sxs),
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
              // 已导入的那几条：勾选框禁用 + 整行压暗（`McpModals.tsx:100-112`）。
              // 勾了也没用的东西不该还能勾。
              Opacity(
                opacity: it.alreadyImported ? 0.5 : 1,
                child: Padding(
                  padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Checkbox(
                        value:
                            it.alreadyImported ||
                            _c.selected.contains(it.name),
                        visualDensity: VisualDensity.compact,
                        onChanged: (_c.importing || it.alreadyImported)
                            ? null
                            : (_) => _c.toggleSelect(it.name),
                      ),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            // 第一行：名字 + 传输 + 来源 agent 徽标 + 已导入
                            //（`McpModals.tsx:118-144`）。`foundInAgents` 早就解析
                            // 进来了，之前只是没上屏。
                            Wrap(
                              spacing: AidogSpace.sxs,
                              crossAxisAlignment: WrapCrossAlignment.center,
                              children: [
                                Text(
                                  it.name,
                                  style: AidogType.label.copyWith(
                                    color: AidogTheme.of(context).c.fg,
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                                MiniBadge(
                                  text: it.transport,
                                  color: AidogTheme.of(context).c.fg3,
                                ),
                                for (final a in it.foundInAgents)
                                  MiniBadge(
                                    text: t.t('mcp.agent.$a'),
                                    color: AidogTheme.of(context).c.fg3,
                                  ),
                                if (it.alreadyImported)
                                  Text(
                                    t.t('mcp.alreadyImported'),
                                    style: AidogType.micro.copyWith(
                                      color: AidogTheme.of(context).c.ok,
                                    ),
                                  ),
                              ],
                            ),
                            // 第二行：跑的是什么（stdio 显命令 + 首参，
                            // http / sse 显 url）。勾之前得看得出这条 MCP 是什么。
                            Text(
                              mcpSummaryOf(
                                transport: it.transport,
                                command: it.command,
                                args: it.args,
                                url: it.url,
                              ),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: AidogType.micro.copyWith(
                                color: AidogTheme.of(context).c.fg3,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
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
          // 列表回传的是**脱敏值**（`***`），用户照原样保存就把字面 `***` 写进
          // 配置、原密钥丢失。React 把这句提示写在标题旁（`Mcp/primitives.tsx:220-225`）。
          Expanded(child: TileMeta('$label（${t.t('mcp.maskedHint')}）')),
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
              // key 只用下标，**不能把输入内容拼进去** —— 那样每敲一个字符 key
              // 就变一次，State 跟着重建，等于没修。删掉某一行时下标会前移、
              // State 被复用到新的那行上，靠 `didUpdateWidget` 里的值比对同步回来。
              Expanded(
                child: KeptTextField(
                  key: ValueKey('kv-k-$i'),
                  value: rows[i].k,
                  onChanged: (v) => rows[i].k = v,
                ),
              ),
              const SizedBox(width: AidogSpace.sxs),
              Expanded(
                child: KeptTextField(
                  key: ValueKey('kv-v-$i'),
                  value: rows[i].v,
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
            // 标签独立成行，不塞在 hint 里（`Mcp/primitives.tsx` 的 McpForm）：
            // hint 一旦填了内容就没了，回头看不出这格是什么字段。
            TileMeta(t.t('mcp.field.name')),
            KeptTextField(
              key: const Key('mcp-name'),
              value: f.name,
              hint: t.t('mcp.field.name'),
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
              TileMeta(t.t('mcp.field.command')),
              KeptTextField(
                key: const Key('mcp-command'),
                value: f.command,
                hint: t.t('mcp.field.command'),
                onChanged: (v) => f.command = v,
              ),
              const SizedBox(height: AidogSpace.sxs),
              TileMeta(t.t('mcp.field.args')),
              KeptTextField(
                key: const Key('mcp-args'),
                value: f.argsText,
                maxLines: 3,
                hint: t.t('mcp.field.args'),
                onChanged: (v) => f.argsText = v,
              ),
              const SizedBox(height: AidogSpace.ssm),
              _kvEditor(t, t.t('mcp.field.env'), f.envRows),
            ] else ...[
              TileMeta(t.t('mcp.field.url')),
              KeptTextField(
                key: const Key('mcp-url'),
                value: f.url,
                hint: t.t('mcp.field.url'),
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

  /// 分享面板复用平台那一套（React 也是同一个 `ShareModal`，
  /// `McpModals.tsx:331-341`）：yaml / json / base64 / url 四格式 + 二维码。
  ///
  /// 原先这里自己画了一个只读框，复制出去的是 `Map.toString()` 的产物 ——
  /// `{mcpServers: {x: {command: npx}}}` 这种**无引号串既不是 JSON 也不是 YAML**，
  /// 接收端按哪种解析都会失败，等于分享功能整条是坏的。
  Widget _shareCard(I18nController t) {
    final data = _c.shareData!;
    return SharePanel(
      share: data.share,
      title: data.name,
      urlScheme: 'aidog://mcp/import',
      titleKey: 'mcp.share.title',
      warningKey: 'mcp.share.warning',
      onToast: _c.showToast,
      onClose: _c.closeShare,
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
              // codex 只支持 stdio。不支持的组合直接禁用并把原因写进 tooltip
              //（`Mcp/primitives.tsx:95-114`）——原先恒可点，点下去才弹错误。
              // 30×30 图标按钮（`Mcp/primitives.tsx:95-125`）：原先是文字按钮，
              // 行尾被两段文案撑得很宽。
              for (final a in kMcpAgents)
                AgentIconButton(
                  agent: a,
                  enabled: server.enabledAgents.contains(a),
                  supported: mcpAgentSupported(server.transport, a),
                  tooltip: mcpAgentSupported(server.transport, a)
                      ? t.t('mcp.agent.$a')
                      : t.t('mcp.unsupportedTransportTip', {
                          'transport': server.transport,
                        }),
                  onTap: (busy || !mcpAgentSupported(server.transport, a))
                      ? null
                      : () => onToggleAgent(a),
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
