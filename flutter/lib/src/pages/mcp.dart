/// MCP 页界面（票 I09），对应 `src/pages/Mcp.tsx` 的主列表 + 五个弹窗。
///
/// 五个弹窗（扫描导入 / 粘贴导入 / 删除确认 / 编辑 / 分享）与票 I07 同一做法：
/// 页面 state 的一部分，不是 route，widget 测试直接断言。色值一律走主题。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../deep_link.dart';
import '../shell/app_shell.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'invoke.dart';
import 'mcp_logic.dart';
import 'mini_select.dart';
import 'settings/schema_config_page.dart' show JsonField;
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
  StreamSubscription<DeepLinkPayload>? _deepLinkSub;

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
    final pending = deepLinks.takePending('mcp');
    if (pending != null) _consumeDeepLink(pending);
    _deepLinkSub = deepLinks.subscribe('mcp', _consumeDeepLink);
  }

  void _consumeDeepLink(DeepLinkPayload payload) {
    if (payload.action != 'import' || payload.data.isEmpty) return;
    unawaited(_c.openDeepLinkImport(payload.data));
  }

  @override
  void dispose() {
    _deepLinkSub?.cancel();
    super.dispose();
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
          // 页标题 22 w700 ls0（`McpView.tsx:21`）。
          titleStyle: AidogType.display.copyWith(
            fontSize: 22,
            fontWeight: FontWeight.w700,
            letterSpacing: 0,
          ),
          // 计数在标题**同一行右侧** 13 tertiary（`McpView.tsx:24-26`）。
          subtitle: ltr('${_c.servers.length}'),
          subtitleStyle: AidogType.caption.copyWith(
            fontSize: 13,
            color: AidogTheme.of(context).c.fg3,
          ),
          inlineSubtitle: true,
          trailing: Wrap(
            // 顶栏间距 12（`McpView.tsx:20`）。
            spacing: 12,
            runSpacing: 12,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              // 顺序照 `McpView.tsx:28-59`：添加 → 粘贴导入 → 重新同步 →
              // 扫描导入。四颗都是 shadcn `<Button>` 默认档（36 高 / px-4 /
              // py-2 / 14），这一栏**没有** fontSize 覆盖。
              //
              // 四颗全部 `disabled={busyKey !== null}`：任何一个动作在忙时整栏锁定。
              SmallButton(
                label: t.t('mcp.add'),
                fontSize: 14,
                padding: (16, 8),
                onTap: _c.busyKey == null ? _c.openAdd : null,
              ),
              SmallButton(
                label: t.t('mcp.pasteImport'),
                fontSize: 14,
                padding: (16, 8),
                onTap: _c.busyKey == null ? () => _c.setPasteOpen(true) : null,
              ),
              SmallButton(
                label: t.t('mcp.resync'),
                fontSize: 14,
                padding: (16, 8),
                // 这颗按钮会重写所有已启用 agent 的配置文件，解释尤其不能省
                //（`Mcp/McpView.tsx:49` 的 `title=`）。
                tooltip: t.t('mcp.resyncHint'),
                onTap: _c.busyKey == null ? _c.resync : null,
              ),
              // React 这颗没写 variant = 默认实心（`McpView.tsx:53-59`），
              // 旁边三颗是 `variant="outline"`，保持描边。
              SmallButton(
                label: t.t('mcp.scanImport'),
                filled: true,
                fontSize: 14,
                padding: (16, 8),
                onTap: _c.busyKey == null ? _c.openScan : null,
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
        // React 的 loading 是**裸文字** 14 tertiary、没有卡面（`McpView.tsx:84-87`）。
        if (_c.loading)
          Text(
            t.t('status.loading'),
            style: AidogType.label.copyWith(
              fontSize: 14,
              color: AidogTheme.of(context).c.fg3,
            ),
          )
        // 空态是 **1px 虚线**框 + r12 + 32 内衬 + 14 tertiary（`McpView.tsx:89-100`），
        // 不是实线卡。
        else if (_c.servers.isEmpty)
          _McpEmptyNote(text: t.t('mcp.empty'))
        else
          Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final s in _c.servers)
                Padding(
                  // 列表间距 8（`McpView.tsx:102`）。
                  padding: const EdgeInsets.only(bottom: 8),
                  child: _McpRow(
                    server: s,
                    busyKey: _c.busyKey,
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
            // 标题 16 w700 / 正文 13 tertiary lh1.5 / 页脚按钮 shadcn 默认档
            //（`McpModals.tsx:212-215,222-232`）。
            titleStyle: _mcpDialogTitleStyle,
            bodyStyle: AidogType.label.copyWith(
              fontSize: 13,
              height: 1.5,
              color: AidogTheme.of(context).c.fg3,
            ),
            buttonFontSize: 14,
            buttonPadding: (16, 8),
            onCancel: _c.cancelDelete,
            onConfirm: _c.delete,
          ),
        if (_c.shareData != null) _shareCard(t),
        // 消息条是**页内常驻**方条：8/12 + r8 + 1px 语义色边 + bg-elevated 底 +
        // 13 语义色字（`McpView.tsx:68-81`），不是浮在窗口顶部的彩色胶囊。
        if (_c.message != null)
          Padding(
            padding: const EdgeInsets.only(top: AidogSpace.ssm),
            child: InlineNote(
              text: _c.message!.text,
              padding: const EdgeInsets.symmetric(
                horizontal: 12,
                vertical: 8,
              ),
              background: AidogTheme.of(context).c.surface2,
              borderColor: _c.message!.ok
                  ? AidogTheme.of(context).c.ok
                  : AidogTheme.of(context).c.bad,
              color: _c.message!.ok
                  ? AidogTheme.of(context).c.ok
                  : AidogTheme.of(context).c.bad,
            ),
          ),
      ],
    );
  }

  /// 三个 `Dialog` 的标题：16 w700（`McpModals.tsx:56,176,243`）。
  static final TextStyle _mcpDialogTitleStyle = AidogType.title.copyWith(
    fontSize: 16,
    fontWeight: FontWeight.w700,
  );

  // React 是普通 `Dialog`（`McpModals.tsx:50-54`，maxWidth 560 / maxHeight 80vh /
  // gap 12）：导入中不许关，右上角自带 ✕。
  Widget _scanCard(I18nController t) {
    final theme = AidogTheme.of(context);
    return AidogModal(
      maxWidth: 560,
      onBarrierTap: _c.importing ? null : _c.closeScan,
      child: ModalCard(
        title: t.t('mcp.scanTitle'),
        titleStyle: _mcpDialogTitleStyle,
        // 标题右侧的条目计数：扫描中显文案，否则显条数
        //（`McpModals.tsx:59-61`）。原先完全没有。
        meta: _c.scanning ? t.t('mcp.scanning') : '${_c.scanItems.length}',
        // 全选 / 反选跟标题**同一行**（`McpModals.tsx:63-81`）：十几条一条条点太慢。
        // 已导入的不参与（它们本来就勾不动）。扫描中 / 导入中禁用。
        titleTrailing: SmallButton(
          label: t.t('mcp.toggleAll'),
          fontSize: 14,
          padding: (16, 8),
          onTap: (_c.scanning || _c.importing || _c.scanItems.isEmpty)
              ? null
              : _c.toggleSelectAll,
        ),
        onClose: _c.importing ? null : _c.closeScan,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 列表区 50vh 上限、自己滚（`McpModals.tsx:84`）；
            // 原先不限高，条目一多就把页脚顶出视野。
            ConstrainedBox(
              constraints: BoxConstraints(
                maxHeight: MediaQuery.sizeOf(context).height * 0.5,
              ),
              child: SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (_c.scanning)
                      _scanNote(theme, t.t('status.loading'))
                    else if (_c.scanItems.isEmpty)
                      _scanNote(theme, t.t('mcp.scanEmpty'))
                    else
                      for (final it in _c.scanItems)
                        Padding(
                          // 条目间距 6（`McpModals.tsx:84`）。
                          padding: const EdgeInsets.only(bottom: 6),
                          child: _ScanItemRow(
                            item: it,
                            checked:
                                it.alreadyImported ||
                                _c.selected.contains(it.name),
                            onTap: (_c.importing || it.alreadyImported)
                                ? null
                                : () => _c.toggleSelect(it.name),
                          ),
                        ),
                  ],
                ),
              ),
            ),
            // 页脚 gap 8 + marginTop 4（`McpModals.tsx:152`）。
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  fontSize: 14,
                  padding: (16, 8),
                  onTap: _c.importing ? null : _c.closeScan,
                ),
                const SizedBox(width: 8),
                SmallButton(
                  // 弹窗主按钮实心、取消描边（`McpModals.tsx:153-164`）。
                  filled: true,
                  fontSize: 14,
                  padding: (16, 8),
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
  }

  /// 扫描列表里的「扫描中 / 无结果」一句话（`McpModals.tsx:85-92`：24 内衬 + 13）。
  Widget _scanNote(AidogTheme theme, String text) => Padding(
    padding: const EdgeInsets.all(24),
    child: Center(
      child: Text(
        text,
        style: AidogType.label.copyWith(fontSize: 13, color: theme.c.fg3),
      ),
    ),
  );

  // React 是普通 `Dialog`（`McpModals.tsx:170-174`，maxWidth 560）：导入中不许关，
  // 右上角自带 ✕。
  Widget _pasteCard(I18nController t) => AidogModal(
    maxWidth: 560,
    onBarrierTap: _c.pasteBusy ? null : () => _c.setPasteOpen(false),
    child: ModalCard(
      title: t.t('mcp.pasteTitle'),
      titleStyle: _mcpDialogTitleStyle,
      onClose: _c.pasteBusy ? null : () => _c.setPasteOpen(false),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          // 粘的是一整份 mcpServers 配置。React 那边是 `JsonCodeEditor`
          //（高度 220–360px，`McpModals.tsx:183-189`）：这里直接复用 schema
          // 设置页同一个 JSON 编辑器（语法高亮 + 行号 + 折叠 + 格式化），
          // 不另造一份。高度取 React 区间中点 ≈ 290。
          JsonField(
            key: const Key('mcp-paste'),
            description: t.t('mcp.pasteHint'),
            height: 290,
            syncExternal: false,
            hint: '{\n  "mcpServers": {\n    "filesystem": {\n      "command": "npx",\n      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"]\n    }\n  }\n}',
            onSubmitted: (_) {},
            onChanged: _c.setPasteText,
          ),
          // 页脚 gap 8 + marginTop 4（`McpModals.tsx:190`）。
          const SizedBox(height: 16),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              SmallButton(
                label: t.t('action.cancel'),
                fontSize: 14,
                padding: (16, 8),
                onTap: _c.pasteBusy ? null : () => _c.setPasteOpen(false),
              ),
              const SizedBox(width: 8),
              SmallButton(
                filled: true,
                fontSize: 14,
                padding: (16, 8),
                // `mcp.import` 带 `{{count}}` 占位，这里没有条数可填 ——
                // 沿用通用确认文案（React 那处走的是 i18next 的无参回落）。
                label: _c.pasteBusy
                    ? t.t('mcp.importing')
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

  Widget _kvEditor(I18nController t, String label, List<KvRow> rows) {
    final theme = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // 标题与提示是**两段**：标签 12 secondary + 提示 11 tertiary 左移 6
        //（`Mcp/primitives.tsx:220-225`），原先拼成一个 12.5 的串。
        // 列表回传的是**脱敏值**（`***`），用户照原样保存就把字面 `***` 写进
        // 配置、原密钥丢失 —— 这句不能省。
        Text.rich(
          TextSpan(
            children: [
              TextSpan(text: label),
              TextSpan(
                text: '      (${t.t('mcp.maskedHint')})',
                style: AidogType.caption.copyWith(
                  fontSize: 11,
                  color: theme.c.fg3,
                ),
              ),
            ],
          ),
          style: AidogType.caption.copyWith(fontSize: 12, color: theme.c.fg2),
        ),
        for (var i = 0; i < rows.length; i++)
          Padding(
            // 行与行 6、行内 6（`Mcp/primitives.tsx:219,227`）。
            padding: const EdgeInsets.only(top: 6),
            child: Row(
              children: [
                // key 只用下标，**不能把输入内容拼进去** —— 那样每敲一个字符 key
                // 就变一次，State 跟着重建，等于没修。删掉某一行时下标会前移、
                // State 被复用到新的那行上，靠 `didUpdateWidget` 里的值比对同步回来。
                // key 列 flex 1、value 列 flex 1.4 且**等宽**
                //（`Mcp/primitives.tsx:228-239`）。
                Expanded(
                  flex: 10,
                  child: KeptTextField(
                    key: ValueKey('kv-k-$i'),
                    value: rows[i].k,
                    hint: 'KEY',
                    onChanged: (v) => rows[i].k = v,
                  ),
                ),
                const SizedBox(width: 6),
                Expanded(
                  flex: 14,
                  child: KeptTextField(
                    key: ValueKey('kv-v-$i'),
                    value: rows[i].v,
                    hint: '***',
                    monospace: true,
                    onChanged: (v) => rows[i].v = v,
                  ),
                ),
                const SizedBox(width: 6),
                SmallButton(
                  // 删除键是 `outline` 变体的 `×`、6px 10px
                  //（`Mcp/primitives.tsx:240-247`），不是红色的「删除」文字键。
                  label: '×',
                  tooltip: t.t('action.delete'),
                  padding: (10, 6),
                  onTap: () => setState(() => rows.removeAt(i)),
                ),
              ],
            ),
          ),
        // 「+ 添加」排在**所有行之下**、左对齐、12 / 5px 10px
        //（`Mcp/primitives.tsx:250-256`），原先排在标题行右端。
        Padding(
          padding: const EdgeInsets.only(top: 6),
          child: Align(
            alignment: AlignmentDirectional.centerStart,
            child: SmallButton(
              label: '+ ${t.t('mcp.addRow')}',
              fontSize: 12,
              padding: (10, 5),
              onTap: () => setState(() => rows.add(KvRow('', ''))),
            ),
          ),
        ),
      ],
    );
  }

  Widget _editCard(I18nController t) {
    final f = _c.editForm;
    // React 是普通 `Dialog`（`McpModals.tsx:237-241`，maxWidth 560 /
    // maxHeight 80vh / gap 12），点遮罩可关，右上角自带 ✕。
    return AidogModal(
      maxWidth: 560,
      onBarrierTap: _c.closeEdit,
      child: ModalCard(
        title: _c.editTarget == null ? t.t('mcp.add') : t.t('mcp.edit'),
        titleStyle: _mcpDialogTitleStyle,
        onClose: _c.closeEdit,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 字段区自己滚（`McpModals.tsx:249` 的 `overflow: auto`）；
            // 原先不限高，stdio + 一堆 env 行就把页脚顶出视野。
            ConstrainedBox(
              constraints: BoxConstraints(
                maxHeight: MediaQuery.sizeOf(context).height * 0.6,
              ),
              child: SingleChildScrollView(
                // React 这块右侧留了 4 给滚动条（`McpModals.tsx:249`）。
                padding: const EdgeInsetsDirectional.only(end: 4),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    // 标签独立成行，不塞在 hint 里：hint 一旦填了内容就没了，
                    // 回头看不出这格是什么字段。标签 12 secondary、
                    // 标签↔输入 4、字段之间 10（`McpModals.tsx:249-250`）。
                    FieldLabel(t.t('mcp.field.name'), fontSize: 12),
                    const SizedBox(height: 4),
                    KeptTextField(
                      key: const Key('mcp-name'),
                      value: f.name,
                      hint: t.t('mcp.field.name'),
                      onChanged: (v) => f.name = v,
                    ),
                    const SizedBox(height: 10),
                    FieldLabel(t.t('mcp.field.transport'), fontSize: 12),
                    const SizedBox(height: 4),
                    // React 是**整宽** `Select` 下拉、13（`McpModals.tsx:259-271`），
                    // 原先跟标签挤在一个 Wrap 里。
                    Align(
                      alignment: AlignmentDirectional.centerStart,
                      child: MiniSelect(
                        key: const ValueKey('mcp-transport'),
                        value: f.transport,
                        fontSize: 13,
                        options: const ['stdio', 'http', 'sse'],
                        onChanged: (v) => setState(() => f.transport = v!),
                      ),
                    ),
                    const SizedBox(height: 10),
                    // stdio 用 command + args；http / sse 用 url + headers。
                    if (f.transport == 'stdio') ...[
                      FieldLabel(t.t('mcp.field.command'), fontSize: 12),
                      const SizedBox(height: 4),
                      KeptTextField(
                        key: const Key('mcp-command'),
                        value: f.command,
                        hint: t.t('mcp.field.command'),
                        onChanged: (v) => f.command = v,
                      ),
                      const SizedBox(height: 10),
                      FieldLabel(t.t('mcp.field.args'), fontSize: 12),
                      const SizedBox(height: 4),
                      // `<Textarea minHeight 64>` + 等宽（`McpModals.tsx:285-289`）。
                      SizedBox(
                        height: 64,
                        child: KeptTextField(
                          key: const Key('mcp-args'),
                          value: f.argsText,
                          maxLines: null,
                          monospace: true,
                          hint: t.t('mcp.field.args'),
                          onChanged: (v) => f.argsText = v,
                        ),
                      ),
                      const SizedBox(height: 10),
                      _kvEditor(t, t.t('mcp.field.env'), f.envRows),
                    ] else ...[
                      FieldLabel(t.t('mcp.field.url'), fontSize: 12),
                      const SizedBox(height: 4),
                      KeptTextField(
                        key: const Key('mcp-url'),
                        value: f.url,
                        hint: t.t('mcp.field.url'),
                        onChanged: (v) => f.url = v,
                      ),
                      const SizedBox(height: 10),
                      _kvEditor(t, t.t('mcp.field.headers'), f.headersRows),
                    ],
                  ],
                ),
              ),
            ),
            // 页脚 gap 8 + marginTop 4（`McpModals.tsx:315`）。
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  fontSize: 14,
                  padding: (16, 8),
                  onTap: _c.closeEdit,
                ),
                const SizedBox(width: 8),
                SmallButton(
                  filled: true,
                  fontSize: 14,
                  padding: (16, 8),
                  label: _c.busyKey != null
                      ? t.t('mcp.saving')
                      : t.t('action.save'),
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

/// transport 彩色徽标（`Mcp/constants.ts:18::transportStyle`）：
/// http/sse 走 accent 系（accentWash 底 + accentText 字），stdio 中性
/// （surface 底 + fg3 字）。10 w600 全大写 ls 0.3、无边框（`Mcp/primitives.tsx:184-201`）。
class _TransportBadge extends StatelessWidget {
  const _TransportBadge({required this.transport});

  final String transport;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final remote = transport == 'http' || transport == 'sse';
    return Align(
      alignment: AlignmentDirectional.centerStart,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
        decoration: BoxDecoration(
          color: remote ? theme.c.accentWash : theme.c.surface,
          borderRadius: BorderRadius.circular(6),
        ),
        child: Text(
          transport.toUpperCase(),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: AidogType.micro.copyWith(
            fontSize: 10,
            letterSpacing: 0.3,
            fontWeight: FontWeight.w600,
            color: remote ? theme.c.accentText : theme.c.fg3,
          ),
        ),
      ),
    );
  }
}

class _McpRow extends StatelessWidget {
  const _McpRow({
    required this.server,
    required this.busyKey,
    required this.onToggleAgent,
    required this.onEdit,
    required this.onShare,
    required this.onDelete,
  });

  final McpServerInfo server;

  /// React 侧是单值 `busyKey`（`Mcp/primitives.tsx`），按前缀精确到「哪个服务器
  /// 的哪个动作」：`edit::<name>` / `del::<name>` / `<name>::<agent>`。
  /// 原先把 `busyKey != null` 整行传下来，一个服务器在忙会把**所有**行的
  /// 编辑 / 删除 / 开关全部锁死 —— 过度禁用。
  final String? busyKey;
  final void Function(String agent) onToggleAgent;
  final VoidCallback onEdit;
  final VoidCallback onShare;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Tile(
      // React MCP 行卡 10/12（Mcp/primitives.tsx:47）。
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      child: Row(
        children: [
          Expanded(
            flex: 3,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                // React：transport 徽标跟在名字**旁边**
                //（`Mcp/primitives.tsx:50-53`），不是独立一列。
                Wrap(
                  spacing: 8,
                  runSpacing: 2,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text(
                      server.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AidogType.body.copyWith(
                        fontSize: 14,
                        // React 这处是内联 14 w600、字距 normal
                        //（`Mcp/primitives.tsx:52`），不带 body 档的 -0.16。
                        letterSpacing: 0,
                        color: theme.c.fg,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    _TransportBadge(transport: server.transport),
                  ],
                ),
                Padding(
                  // summary 12 + marginTop 3（Mcp/primitives.tsx:55-59）。
                  padding: const EdgeInsets.only(top: 3),
                  child: Text(
                    server.summary,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    // 摘要行 12（`Mcp/primitives.tsx:58`）。
                    style: AidogType.caption.copyWith(
                      fontSize: 12,
                      color: theme.c.fg3,
                    ),
                  ),
                ),
                // 行内 env chips（`Mcp/primitives.tsx:67-86`）：k=v 等宽小字胶囊，
                // 值由后端 mask_env 打码，原样展示即可。
                if (server.env.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Wrap(
                      spacing: 6,
                      runSpacing: 2,
                      children: [
                        for (final e in server.env.entries)
                          Container(
                            // 胶囊底与 React 同款：bg 底 + 1px 5px 内衬 + 4 圆角
                            //（`Mcp/primitives.tsx:71-84`）。
                            padding: const EdgeInsets.symmetric(
                              horizontal: 5,
                              vertical: 1,
                            ),
                            decoration: BoxDecoration(
                              color: theme.c.bg,
                              borderRadius: BorderRadius.circular(4),
                            ),
                            child: Text(
                              '${e.key}=${e.value}',
                              style: AidogType.numSm.copyWith(
                                fontSize: 10,
                                color: theme.c.fg3,
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
          // 主区与操作区之间 10（卡内 gap，`Mcp/primitives.tsx:46`）。
          const SizedBox(width: 10),
          Wrap(
            // 行尾操作组 gap 6（`Mcp/primitives.tsx:90`）。
            spacing: 6,
            runSpacing: 6,
            crossAxisAlignment: WrapCrossAlignment.center,
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
                  onTap:
                      (busyKey == '${server.name}::$a' ||
                          !mcpAgentSupported(server.transport, a))
                      ? null
                      : () => onToggleAgent(a),
                ),
              // 编辑 / 分享 / 删除：30×30 图标按钮（15px 图标，
              // `Mcp/primitives.tsx:128-175`），不是文字按钮。
              _RowIconButton(
                icon: Icons.edit_outlined,
                tooltip: t.t('action.edit'),
                onTap: (busyKey?.startsWith('edit::${server.name}') ?? false)
                    ? null
                    : onEdit,
              ),
              _RowIconButton(
                icon: Icons.share_outlined,
                tooltip: t.t('mcp.share'),
                onTap: onShare,
              ),
              _RowIconButton(
                icon: Icons.delete_outline,
                tooltip: t.t('action.delete'),
                danger: true,
                onTap: (busyKey?.startsWith('del::${server.name}') ?? false)
                    ? null
                    : onDelete,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// MCP 行尾的 30×30 图标按钮（对齐 React 的 `size="icon"` outline 按钮，
/// `Mcp/primitives.tsx:128-175`）。
class _RowIconButton extends StatelessWidget {
  const _RowIconButton({
    required this.icon,
    required this.tooltip,
    this.onTap,
    this.danger = false,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback? onTap;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Tooltip(
      message: tooltip,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        child: Container(
          width: 30,
          height: 30,
          alignment: Alignment.center,
          // React 这三颗是 `<Button variant="outline" size="icon">`：有描边、
          // 有底（`Mcp/primitives.tsx:131-137,148-155,164-171`）。
          // 原先是裸图标，跟旁边有边框的 agent 按钮不是一套。
          decoration: BoxDecoration(
            color: theme.c.surface,
            border: Border.all(color: theme.c.line),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Icon(
            icon,
            size: 15,
            color: onTap == null
                ? theme.c.fg3
                : danger
                ? theme.c.bad
                : theme.c.fg2,
          ),
        ),
      ),
    );
  }
}

/// MCP 列表空态：**1px 虚线**框 + r12 + 32 内衬 + 14 tertiary
///（`McpView.tsx:89-100`）。[CenteredNote] 那张是实线卡 + 阴影，不是这一款。
class _McpEmptyNote extends StatelessWidget {
  const _McpEmptyNote({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return CustomPaint(
      painter: DashedBorder(color: theme.c.line, radius: AidogRadius.md),
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Center(
          child: Text(
            text,
            style: AidogType.label.copyWith(
              fontSize: 14,
              color: theme.c.fg3,
            ),
          ),
        ),
      ),
    );
  }
}

/// 扫描列表的一行（`McpModals.tsx:98-146`）：整行是个 `<label>`，点哪都能勾 ——
/// `8px 10px` 内衬、r8、1px 中性边，已导入的换 bg-elevated 底并压暗。
class _ScanItemRow extends StatelessWidget {
  const _ScanItemRow({
    required this.item,
    required this.checked,
    required this.onTap,
  });

  final McpScanItem item;
  final bool checked;

  /// null = 已导入 / 正在导入，勾不动。
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final done = item.alreadyImported;
    return Opacity(
      opacity: done ? 0.5 : 1,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          decoration: BoxDecoration(
            color: done ? theme.c.surface2 : Colors.transparent,
            border: Border.all(color: theme.c.line),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Checkbox(
                value: checked,
                visualDensity: VisualDensity.compact,
                materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                activeColor: theme.c.accent,
                onChanged: onTap == null ? null : (_) => onTap!(),
              ),
              // 行内 gap 10（`McpModals.tsx:103`）。
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    // 第一行：名字 + 传输 + 来源 agent 徽标 + 已导入
                    //（`McpModals.tsx:119-141`），行内 gap 6。
                    Wrap(
                      spacing: 6,
                      runSpacing: 4,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
                        Text(
                          item.name,
                          // 继承容器的 13 + w600（`McpModals.tsx:110,120`）。
                          style: AidogType.label.copyWith(
                            fontSize: 13,
                            color: theme.c.fg,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        _TransportBadge(transport: item.transport),
                        // 来源 agent chip：r4 / 1px 5px / bg-elevated 平底 /
                        // 无描边 / 字 tertiary（`McpModals.tsx:122-135`）。
                        for (final a in item.foundInAgents)
                          MiniBadge(
                            text: t.t('mcp.agent.$a'),
                            color: theme.c.fg3,
                            background: theme.c.surface2,
                            borderColor: Colors.transparent,
                            padX: 5,
                            radius: 4,
                          ),
                        if (done)
                          Text(
                            t.t('mcp.alreadyImported'),
                            // 10 success（`McpModals.tsx:137`）。
                            style: AidogType.micro.copyWith(
                              fontSize: 10,
                              letterSpacing: 0,
                              color: theme.c.ok,
                            ),
                          ),
                      ],
                    ),
                    // 第二行：跑的是什么（stdio 显命令 + 首参，http / sse 显 url）。
                    // 勾之前得看得出这条 MCP 是什么。11 tertiary + 上距 2
                    //（`McpModals.tsx:142`）。
                    Padding(
                      padding: const EdgeInsets.only(top: 2),
                      child: Text(
                        mcpSummaryOf(
                          transport: item.transport,
                          command: item.command,
                          args: item.args,
                          url: item.url,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AidogType.micro.copyWith(
                          letterSpacing: 0,
                          color: theme.c.fg3,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
