/// 分享面板（票 I18b），对应 React 的 `src/components/platforms/ShareModal.tsx`。
///
/// 打开即自动复制一次 + 手动复制按钮；格式切换 URL(默认) / YAML / JSON / Base64。
/// 剪贴板走 [native.writeText]（`platform.dart`），与 React 的 Tauri 插件同一条路。
///
/// **与 React 的一处差异**：那边在 URL 格式下还画一张二维码（`qrcode` 包）。Dart 侧没有
/// 已装的二维码生成库，为一张图引一个新依赖不划算 —— 链接本身照出，复制照旧可用。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'platform_card_bits.dart';
import 'ui_bits.dart';

class SharePanel extends StatefulWidget {
  const SharePanel({
    super.key,
    required this.share,
    required this.title,
    required this.onToast,
    required this.onClose,
    this.urlScheme,
    this.copy = native.writeText,
  });

  /// 可分享配置对象（`platform_share_export` 的返回，含明文 api_key）。
  final Map<String, Object?> share;

  /// 面板标题后缀（平台名）。
  final String title;
  final void Function(String text, {required bool ok}) onToast;
  final VoidCallback onClose;

  /// 给了就多一个 URL 格式（深链 `aidog://platform/import?data=<base64>`）并默认选中。
  final String? urlScheme;

  /// 写剪贴板。测试注入假实现（真写会动用户的剪贴板）。
  final Future<void> Function(String text) copy;

  @override
  State<SharePanel> createState() => _SharePanelState();
}

class _SharePanelState extends State<SharePanel> {
  late ShareFormat _format;
  bool _copied = false;

  List<ShareFormat> get _formats => widget.urlScheme != null
      ? ShareFormat.values
      : [ShareFormat.yaml, ShareFormat.json, ShareFormat.base64];

  @override
  void initState() {
    super.initState();
    _format = widget.urlScheme != null ? ShareFormat.url : ShareFormat.yaml;
    // 打开即自动复制一次（`ShareModal.tsx:129`）。
    WidgetsBinding.instance.addPostFrameCallback((_) => _doCopy(auto: true));
  }

  String get _text =>
      formatShare(widget.share, _format, urlScheme: widget.urlScheme);

  Future<void> _doCopy({required bool auto}) async {
    final t = AidogI18n.of(context);
    try {
      await widget.copy(_text);
      if (!mounted) return;
      widget.onToast(
        auto
            ? t.t('platform.share.autoCopied')
            : t.t('platform.share.copied'),
        ok: true,
      );
      setState(() => _copied = true);
    } catch (_) {
      if (!mounted) return;
      widget.onToast(t.t('platform.share.copyFail'), ok: false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: '${t.t('platform.share.title')} · ${widget.title}',
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 安全警示：分享内容含明文 API Key。
            Container(
              padding: const EdgeInsets.symmetric(
                horizontal: AidogSpace.smd,
                vertical: AidogSpace.ssm,
              ),
              decoration: BoxDecoration(
                border: Border.all(color: theme.c.bad),
                borderRadius: BorderRadius.circular(AidogRadius.sm),
              ),
              child: Text(
                t.t('platform.share.warning'),
                style: AidogType.caption.copyWith(color: theme.c.bad),
              ),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Wrap(
              spacing: AidogSpace.ssm,
              children: [
                for (final f in _formats)
                  SmallButton(
                    label: t.t('platform.share.format.${f.name}'),
                    active: _format == f,
                    onTap: () => setState(() => _format = f),
                  ),
              ],
            ),
            const SizedBox(height: AidogSpace.ssm),
            Container(
              constraints: const BoxConstraints(minHeight: 120, maxHeight: 260),
              width: double.infinity,
              padding: const EdgeInsets.all(AidogSpace.smd),
              decoration: BoxDecoration(
                color: theme.c.surface2,
                border: Border.all(color: theme.c.line),
                borderRadius: BorderRadius.circular(AidogRadius.sm),
              ),
              child: SingleChildScrollView(
                child: SelectableText(
                  _text,
                  style: AidogType.numSm.copyWith(color: theme.c.fg),
                ),
              ),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.close'),
                  onTap: widget.onClose,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: _copied
                      ? t.t('platform.share.copiedBtn')
                      : t.t('platform.share.copyBtn'),
                  onTap: () => _doCopy(auto: false),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
