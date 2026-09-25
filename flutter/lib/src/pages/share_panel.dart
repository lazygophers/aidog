/// 分享面板（票 I18b），对应 React 的 `src/components/platforms/ShareModal.tsx`。
///
/// 打开即自动复制一次 + 手动复制按钮；格式切换 URL(默认) / YAML / JSON / Base64。
/// 剪贴板走 [native.writeText]（`platform.dart`），与 React 的 Tauri 插件同一条路。
///
/// 给了 `urlScheme` 就在正文下方画一张深链二维码（`ShareModal.tsx:221-262`）：
/// 链接长度超 [kQrMaxUrlLen] 就不画，换成「内容过长」提示（与 React 同一条判据）。
library;

import 'package:flutter/material.dart';
import 'package:pretty_qr_code/pretty_qr_code.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../shell/theme.dart';
import 'platform_card_bits.dart';
import 'ui_bits.dart';

/// 深链超过这个长度就不画二维码（`ShareModal.tsx:23::QR_MAX_URL_LEN`）。
const int kQrMaxUrlLen = 2900;

class SharePanel extends StatefulWidget {
  const SharePanel({
    super.key,
    required this.share,
    required this.title,
    required this.onToast,
    required this.onClose,
    this.urlScheme,
    this.titleKey = 'platform.share.title',
    this.warningKey = 'platform.share.warning',
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

  /// 标题 / 警示语的文案 key。平台以外的调用点各自传自己的
  /// （`ShareModal.tsx:36,38`：mcp 传 `mcp.share.*`，skill 传 `skills.share.*`）。
  final String titleKey;
  final String warningKey;

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

  /// 二维码里放的始终是深链（与当前选中的格式无关，`ShareModal.tsx:87-91`）。
  /// 没给 urlScheme、或链接过长 → null（调用方画降级提示）。
  String? get _deepLink {
    final scheme = widget.urlScheme;
    if (scheme == null) return null;
    final url = formatShare(widget.share, ShareFormat.url, urlScheme: scheme);
    return url.length > kQrMaxUrlLen ? null : url;
  }

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
    // React 侧是普通 `Dialog`（`ShareModal.tsx:151`，maxWidth 560），点遮罩可关。
    return AidogModal(
      maxWidth: 560,
      onBarrierTap: widget.onClose,
      child: ModalCard(
        // `DialogContent` 自带 ✕（`ui/dialog.tsx:47-50`）。
        onClose: widget.onClose,
        // `padding: "22px 24px"`（`ShareModal.tsx:152`）。
        padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 22),
        title: '${t.t(widget.titleKey)} · ${widget.title}',
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            // 安全警示：分享内容含明文 API Key。
            // `ShareModal.tsx:162-172`：12.5 danger、pad 8/12、带 bg-glass 底。
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              decoration: BoxDecoration(
                color: theme.c.surface2,
                border: Border.all(color: theme.c.bad),
                borderRadius: BorderRadius.circular(AidogRadius.sm),
              ),
              child: Text(
                t.t(widget.warningKey),
                style: AidogType.caption.copyWith(color: theme.c.bad),
              ),
            ),
            const SizedBox(height: AidogSpace.ssm),
            Wrap(
              spacing: AidogSpace.ssm,
              children: [
                for (final f in _formats)
                  // 格式 tab：`ShareModal.tsx:137-147`：12.5、pad 5/14。
                  SmallButton(
                    label: t.t('platform.share.format.${f.name}'),
                    active: _format == f,
                    fontSize: 12.5,
                    padding: (14, 5),
                    onTap: () => setState(() => _format = f),
                  ),
              ],
            ),
            // 格式切换器下方 12（`ShareModal.tsx:180::marginBottom`）。
            const SizedBox(height: 12),
            Container(
              constraints: const BoxConstraints(minHeight: 120, maxHeight: 260),
              width: double.infinity,
              // `ShareModal.tsx:205`：pad 12/14（竖/横）。
              padding: const EdgeInsets.symmetric(
                horizontal: AidogSpace.slg,
                vertical: 12,
              ),
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
            // 二维码区块（仅 urlScheme 存在时出现；超长 → 降级成一行提示）。
            if (widget.urlScheme != null) ...[
              // `ShareModal.tsx:225`：marginTop 12。
              const SizedBox(height: 12),
              Container(
                // `ShareModal.tsx:226`：pad 12/14（竖/横）。
                padding: const EdgeInsets.symmetric(
                  horizontal: AidogSpace.slg,
                  vertical: 12,
                ),
                decoration: BoxDecoration(
                  color: theme.c.surface2,
                  border: Border.all(color: theme.c.line),
                  borderRadius: BorderRadius.circular(AidogRadius.sm),
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      t.t('platform.share.scanToImport'),
                      // `ShareModal.tsx:236`：12.5 w600。
                      style: AidogType.caption.copyWith(
                        color: theme.c.fg2,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: AidogSpace.ssm),
                    if (_deepLink case final link?)
                      SizedBox(
                        width: 160,
                        height: 160,
                        child: PrettyQrView.data(
                          data: link,
                          errorCorrectLevel: QrErrorCorrectLevel.L,
                          // 黑白是二维码的功能色不是主题色：跟着暗色主题走会把
                          // 对比度压到扫不出来（React 那边用的也是 qrcode 包的
                          // 黑白缺省值，`ShareModal.tsx:100`）。
                          decoration: const PrettyQrDecoration(
                            shape: PrettyQrSmoothSymbol(color: Colors.black),
                            background: Colors.white,
                          ),
                        ),
                      )
                    else
                      SizedBox(
                        width: 200,
                        child: Text(
                          t.t('platform.share.qrTooLong'),
                          textAlign: TextAlign.center,
                          style: AidogType.caption.copyWith(color: theme.c.fg2),
                        ),
                      ),
                  ],
                ),
              ),
            ],
            // `ShareModal.tsx:265-270`：上距 18，两颗 13/6/14，主按钮 minWidth 96。
            const SizedBox(height: AidogSpace.sxl),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.close'),
                  ghost: true,
                  fontSize: 13,
                  padding: (14, 6),
                  onTap: widget.onClose,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: _copied
                      ? t.t('platform.share.copiedBtn')
                      : t.t('platform.share.copyBtn'),
                  filled: true,
                  active: true,
                  fontSize: 13,
                  padding: (14, 6),
                  minWidth: 96,
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
