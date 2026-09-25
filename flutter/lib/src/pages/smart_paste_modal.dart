/// 平台「智能识别」弹窗（票 20），对应 `src/components/platforms/SmartPasteModal.tsx`。
///
/// 读剪贴板 / 手动粘贴杂乱文案 → 解析 base_url / 平台 / apikey / 过期时间 →
/// 用户确认后填进添加表单。识别逻辑全在 `platform_paste_logic.dart`（纯函数），
/// 本文件只管渲染与选中态 —— 票 20 的硬要求。
///
/// 浮层沿用 [AidogModal]（与其余 20 多处弹窗同一条路），不自己造遮罩。
/// 色值一律 `AidogTheme.of(context).c.*`，本文件零硬编码颜色。
library;

import 'dart:convert';

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../../platform.dart' as native;
import '../../utils/formatters.dart';
import '../shell/theme.dart';
import 'invoke.dart';
import 'platform_paste_logic.dart';
import 'ui_bits.dart';

/// 协议倾向 → 角标文案。`SmartPasteModal.tsx:56::PROTO_LABEL`。
/// 这是**请求格式**的名字不是平台名，所以不进 registry，也不进 i18n（React 侧同样写死）。
const Map<ParsedProtocol, String> kProtoLabel = {
  ParsedProtocol.anthropic: 'Anthropic',
  ParsedProtocol.openai: 'OpenAI',
  ParsedProtocol.gemini: 'Gemini',
  ParsedProtocol.unknown: 'URL',
};

class SmartPasteModal extends StatefulWidget {
  const SmartPasteModal({
    super.key,
    required this.presets,
    required this.onApply,
    required this.onClose,
    this.onManualEntry,
    this.initialText,
    this.invoke = kernelInvoke,
    this.readClipboard = native.readText,
    this.protocolLabels = const {},
  });

  /// 识别用的 preset 清单（`ProtocolMetaTable.pastePresets`，全部来自 registry）。
  final List<PastePresetRef> presets;

  final void Function(SmartPasteApplyResult r) onApply;
  final VoidCallback onClose;

  /// 「手动填写」：跳过识别直接进空表单。null = 不渲染这颗按钮。
  final VoidCallback? onManualEntry;

  /// 预填文本（deep-link 导入场景）。给了就以它初始化，并**跳过自动读剪贴板**。
  final String? initialText;

  final InvokeFn invoke;

  /// 读剪贴板。抽成参数是为了让 widget 测试不依赖真实剪贴板。
  final Future<String> Function() readClipboard;

  /// 协议 code → 展示名，给分享串命中时那行副标题用。
  final Map<String, String> protocolLabels;

  @override
  State<SmartPasteModal> createState() => _SmartPasteModalState();
}

class _SmartPasteModalState extends State<SmartPasteModal> {
  late final TextEditingController _text = TextEditingController(
    text: widget.initialText ?? '',
  );

  /// 命中的 aidog 分享串；null = 不是分享串，走下面的杂乱文本解析。
  Map<String, Object?>? _share;

  /// 选中的 key / base_url。解析结果一变就按「每协议取第一条 + key 全选」重置。
  List<String> _selKeys = const [];
  List<String> _selUrls = const [];

  ParsedPaste _parsed = const ParsedPaste();

  /// 分享串探测是异步的，用递增序号丢弃过期回包（React 那边的 `cancelled` 闭包同义）。
  int _probeSeq = 0;

  @override
  void initState() {
    super.initState();
    _reparse();
    _probeShare();
    if (widget.initialText == null) _pullClipboard();
  }

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  /// 打开即读剪贴板，自动填进文本框（`SmartPasteModal.tsx:108-122`）。
  /// 读不到 / 没权限就静默放过 —— 手动粘贴本来就兜得住。
  Future<void> _pullClipboard() async {
    final String clip;
    try {
      clip = await widget.readClipboard();
    } on Object {
      return;
    }
    if (!mounted || clip.trim().isEmpty || _text.text.isNotEmpty) return;
    _text.text = clip;
    _onTextChanged(clip);
  }

  void _onTextChanged(String _) {
    _reparse();
    _probeShare();
    setState(() {});
  }

  void _reparse() {
    _parsed = parsePlatformPaste(_text.text, widget.presets);
    // 每个协议取第一条 base_url；key 默认全选（多 key 时即「将批量创建」）。
    final byProto = <ParsedProtocol, String>{};
    for (final b in _parsed.baseUrls) {
      byProto.putIfAbsent(b.protocol, () => b.url);
    }
    _selUrls = byProto.values.toList();
    _selKeys = [..._parsed.apiKeys];
  }

  /// 探测这段文本是不是 aidog 平台分享串（YAML / JSON / Base64 通吃）。
  /// `SmartPasteModal.tsx:73-106`：原串先试，整串像 base64 就再试一次解码后的。
  Future<void> _probeShare() async {
    final seq = ++_probeSeq;
    final trimmed = _text.text.trim();
    if (trimmed.isEmpty) {
      if (mounted) setState(() => _share = null);
      return;
    }
    final candidates = <String>[trimmed];
    if (RegExp(r'^[A-Za-z0-9+/=\s]+$').hasMatch(trimmed) &&
        !trimmed.contains(':')) {
      try {
        final compact = trimmed.replaceAll(RegExp(r'\s+'), '');
        candidates.add(utf8.decode(base64.decode(_pad(compact))));
      } on Object {
        // 不是合法 base64 → 跳过这个候选。
      }
    }
    for (final c in candidates) {
      try {
        final r = await widget.invoke('platform_share_parse', {'text': c});
        if (!mounted || seq != _probeSeq) return;
        setState(() => _share = (r as Map?)?.cast<String, Object?>());
        return;
      } on Object {
        // 不是分享串 → 试下一个候选。
      }
    }
    if (mounted && seq == _probeSeq) setState(() => _share = null);
  }

  static String _pad(String s) {
    final rem = s.length % 4;
    return rem == 0 ? s : s + '=' * (4 - rem);
  }

  /// 切换某条 key 的选中态（多选场景）。
  void _toggleKey(String k) => setState(() {
    _selKeys = _selKeys.contains(k)
        ? [
            for (final x in _selKeys)
              if (x != k) x,
          ]
        : [..._selKeys, k];
  });

  /// 切换某条 base_url。**同协议最多选一条**：选新的会把同协议的旧的顶掉。
  /// `SmartPasteModal.tsx:149::toggleUrl`。
  void _toggleUrl(String url, ParsedProtocol protocol) => setState(() {
    if (_selUrls.contains(url)) {
      _selUrls = [
        for (final u in _selUrls)
          if (u != url) u,
      ];
      return;
    }
    String? sameProto;
    for (final b in _parsed.baseUrls) {
      if (b.protocol == protocol && _selUrls.contains(b.url)) {
        sameProto = b.url;
        break;
      }
    }
    _selUrls = sameProto == null
        ? [..._selUrls, url]
        : [for (final u in _selUrls) u == sameProto ? url : u];
  });

  bool get _hasSelKey =>
      _parsed.apiKeys.length <= 1 ? _selKeys.isNotEmpty : _selKeys.isNotEmpty;

  bool get _canApply =>
      _share != null ||
      _hasSelKey ||
      _selUrls.isNotEmpty ||
      _parsed.platform != null;

  void _apply() {
    final share = _share;
    if (share != null) {
      widget.onApply(SmartPasteApplyResult(fullShare: share));
      widget.onClose();
      return;
    }
    widget.onApply(
      SmartPasteApplyResult(
        platform: _parsed.platform,
        baseUrls: [
          for (final b in _parsed.baseUrls)
            if (_selUrls.contains(b.url)) b,
        ],
        apiKeys: _selKeys,
        expiresAt: _parsed.expiresAt ?? 0,
      ),
    );
    widget.onClose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return AidogModal(
      maxWidth: 540,
      onBarrierTap: widget.onClose,
      child: ModalCard(
        // `DialogContent` 自带 ✕（`ui/dialog.tsx:47-50`）。
        onClose: widget.onClose,
        // `padding: "22px 24px"`（`SmartPasteModal.tsx:187`）。
        padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 22),
        title: t.t('platform.paste.title'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              t.t('platform.paste.hint'),
              // `DialogDescription` = text-sm 14 muted 正体（dialog.tsx:107）。
              style: AidogType.caption.copyWith(
                fontSize: 14,
                color: theme.c.fg2,
              ),
            ),
            const SizedBox(height: AidogSpace.smd),
            TextField(
              controller: _text,
              autofocus: true,
              minLines: 6,
              maxLines: 12,
              decoration: InputDecoration(
                isDense: true,
                hintText: t.t('platform.paste.placeholder'),
                hintStyle: AidogType.label.copyWith(color: theme.c.fg3),
              ),
              style: AidogType.numSm.copyWith(color: theme.c.fg),
              onChanged: _onTextChanged,
            ),
            if (_share != null)
              _ShareHit(share: _share!, protocolLabels: widget.protocolLabels)
            else
              _Detected(
                parsed: _parsed,
                selKeys: _selKeys,
                selUrls: _selUrls,
                onToggleKey: _toggleKey,
                onToggleUrl: _toggleUrl,
              ),
            const SizedBox(height: AidogSpace.smd),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                // 页脚三颗都是 13 正体 pad 6/14（`:374-382`），ghost 侧不描边。
                if (widget.onManualEntry != null) ...[
                  SmallButton(
                    label: t.t('platform.paste.manualEntry'),
                    ghost: true,
                    fontSize: 13,
                    padding: (14, 6),
                    onTap: widget.onManualEntry,
                  ),
                  const SizedBox(width: AidogSpace.ssm),
                ],
                SmallButton(
                  label: t.t('action.cancel'),
                  ghost: true,
                  fontSize: 13,
                  padding: (14, 6),
                  onTap: widget.onClose,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: t.t('platform.paste.apply'),
                  // `SmartPasteModal.tsx:381` 没写 variant = 默认实心；
                  // 同一行的「手动填写」「取消」都是 `variant="ghost"`。
                  filled: true,
                  active: true,
                  fontSize: 13,
                  padding: (14, 6),
                  minWidth: 96,
                  onTap: _canApply ? _apply : null,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// 命中 aidog 分享串时那块高亮区（`SmartPasteModal.tsx:216-239`）。
class _ShareHit extends StatelessWidget {
  const _ShareHit({required this.share, required this.protocolLabels});

  final Map<String, Object?> share;
  final Map<String, String> protocolLabels;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final type = (share['platform_type'] as String?) ?? '';
    return Padding(
      // `SmartPasteModal.tsx:219` 的 marginTop 16。
      padding: const EdgeInsets.only(top: 16),
      child: Container(
        // `SmartPasteModal.tsx:220` 的 padding 12/14（竖/横）。
        padding: const EdgeInsets.symmetric(
          horizontal: AidogSpace.slg,
          vertical: 12,
        ),
        decoration: BoxDecoration(
          color: theme.c.accentWash,
          // React 边是 `var(--accent)`（mono.ts:70 映射 accent-text），
          // 暗色下 accentEdge 白 34% 会比 React 淡一截。
          border: Border.all(color: theme.c.accentText),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            // `SmartPasteModal.tsx:229-237`：13w600 / 12.5 / 12 三行。
            Text(
              t.t('platform.paste.shareDetected'),
              style: AidogType.caption.copyWith(
                fontSize: 13,
                color: theme.c.accentText,
                fontWeight: FontWeight.w600,
              ),
            ),
            Text(
              '${(share['name'] as String?) ?? ''} · '
              '${protocolLabels[type] ?? type}',
              style: AidogType.caption.copyWith(color: theme.c.fg2),
            ),
            Text(
              t.t('platform.paste.shareDetectedHint'),
              style: AidogType.caption.copyWith(
                fontSize: 12,
                color: theme.c.fg3,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// 杂乱文本的识别结果区：平台 / Base URL / Token / 过期时间。
/// `SmartPasteModal.tsx:242-370`。
class _Detected extends StatelessWidget {
  const _Detected({
    required this.parsed,
    required this.selKeys,
    required this.selUrls,
    required this.onToggleKey,
    required this.onToggleUrl,
  });

  final ParsedPaste parsed;
  final List<String> selKeys;
  final List<String> selUrls;
  final void Function(String key) onToggleKey;
  final void Function(String url, ParsedProtocol protocol) onToggleUrl;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final expiresAt = parsed.expiresAt;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        const SizedBox(height: AidogSpace.smd),
        _SectionLabel(t.t('platform.paste.detected')),
        if (!parsed.hasResult)
          Text(
            t.t('platform.paste.empty'),
            // `SmartPasteModal.tsx:247`：13 fg2。
            style: AidogType.caption.copyWith(fontSize: 13, color: theme.c.fg2),
          ),

        // 平台
        if (parsed.hasResult) ...[
          const SizedBox(height: AidogSpace.ssm),
          _SectionLabel(t.t('platform.paste.platform')),
          if (parsed.platform != null)
            _OptionRow(
              selected: true,
              selectedBorder: theme.c.accentText,
              child: Text(
                parsed.platform!.label,
                style: AidogType.caption.copyWith(
                  color: theme.c.accentText,
                  fontWeight: FontWeight.w600,
                ),
              ),
            )
          else
            Text(
              t.t('platform.paste.noPlatform'),
              // `SmartPasteModal.tsx:261`：12.5 fg2。
              style: AidogType.caption.copyWith(color: theme.c.fg2),
            ),
        ],

        // Base URL：按协议多选，每类型最多一个 → 一次建好多端点平台。
        if (parsed.baseUrls.isNotEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          _SectionLabel(
            t.t('platform.paste.baseUrl'),
            hint: t.t('platform.paste.baseUrlMultiHint'),
          ),
          for (final b in parsed.baseUrls)
            _OptionRow(
              selected: selUrls.contains(b.url),
              onTap: () => onToggleUrl(b.url, b.protocol),
              leading: Checkbox(
                value: selUrls.contains(b.url),
                onChanged: (_) => onToggleUrl(b.url, b.protocol),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  _ProtoChip(kProtoLabel[b.protocol]!),
                  const SizedBox(width: AidogSpace.ssm),
                  Flexible(
                    child: Text(
                      b.url,
                      // optRow 正文 13（`SmartPasteModal.tsx:180`）。
                      style: AidogType.caption.copyWith(
                        fontSize: 13,
                        color: theme.c.fg,
                      ),
                    ),
                  ),
                ],
              ),
            ),
        ],

        // Token：单 key 单选（向后兼容），多 key 多选并默认全选 → 批量创建。
        if (parsed.apiKeys.isNotEmpty) ...[
          const SizedBox(height: AidogSpace.ssm),
          _SectionLabel(
            t.t('platform.paste.apiKey'),
            hint: parsed.apiKeys.length > 1
                ? t.t('platform.paste.apiKeyMultiHint', {
                    'n': selKeys.length,
                    'total': parsed.apiKeys.length,
                  })
                : null,
          ),
          // 单 key 走单选（选中之后点它不会取消，与 React 的 `RadioGroup` 同义），
          // 多 key 走多选。
          RadioGroup<String>(
            groupValue: selKeys.isEmpty ? null : selKeys.first,
            onChanged: (v) {
              if (v != null && !selKeys.contains(v)) onToggleKey(v);
            },
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final k in parsed.apiKeys)
                  _OptionRow(
                    selected: selKeys.contains(k),
                    onTap: parsed.apiKeys.length > 1 || !selKeys.contains(k)
                        ? () => onToggleKey(k)
                        : null,
                    leading: parsed.apiKeys.length > 1
                        ? Checkbox(
                            value: selKeys.contains(k),
                            onChanged: (_) => onToggleKey(k),
                          )
                        : Radio<String>(value: k),
                    child: Text(
                      k,
                      // optRow 13 + mono（`SmartPasteModal.tsx:331`）。
                      style: AidogType.numSm.copyWith(
                        fontSize: 13,
                        color: theme.c.fg,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ],

        // 过期时间（社区分享帖常见「即将过期 06-28 23:59」）。
        if (expiresAt != null && expiresAt > 0) ...[
          const SizedBox(height: AidogSpace.ssm),
          _SectionLabel(t.t('platform.expiresAt')),
          _OptionRow(
            selected: true,
            selectedBorder: theme.c.accentText,
            child: Text(
              formatDateTime(expiresAt),
              style: AidogType.caption.copyWith(
                color: theme.c.accentText,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
        ],
      ],
    );
  }
}

/// 小标题（左侧标签 + 可选右侧灰字说明）。
/// `SmartPasteModal.tsx:164-170::labelStyle`：12 w600 uppercase ls0.4。
class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text, {this.hint});

  final String text;
  final String? hint;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
      child: Row(
        children: [
          Text(
            text.toUpperCase(),
            style: AidogType.micro.copyWith(
              fontSize: 12,
              letterSpacing: 0.4,
              color: theme.c.fg2,
              fontWeight: FontWeight.w600,
            ),
          ),
          if (hint != null) ...[
            const Spacer(),
            // `SmartPasteModal.tsx:273`：10.5 w500 fg3，不跟随大写。
            Text(
              hint!,
              style: AidogType.micro.copyWith(
                fontSize: 10.5,
                letterSpacing: 0,
                color: theme.c.fg3,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// 协议小徽标（Base URL 行内的 Anthropic / OpenAI …）。
/// `SmartPasteModal.tsx:288-296`：10.5、pad 1/6、r4、accent-subtle 底、无边。
/// 与 [MiniBadge] 是两种形状（那边 10/r5/12% 底/30% 边），不共用。
class _ProtoChip extends StatelessWidget {
  const _ProtoChip(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AidogSpace.ssm,
        vertical: 1,
      ),
      decoration: BoxDecoration(
        color: theme.c.accentWash,
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        text,
        style: AidogType.micro.copyWith(
          fontSize: 10.5,
          letterSpacing: 0,
          color: theme.c.accentText,
        ),
      ),
    );
  }
}

/// 一条可选项的外框（`SmartPasteModal.tsx:171::optRow`）。
class _OptionRow extends StatelessWidget {
  const _OptionRow({
    required this.selected,
    required this.child,
    this.leading,
    this.onTap,
    this.selectedBorder,
  });

  final bool selected;
  final Widget child;
  final Widget? leading;

  /// null = 只读展示（平台 / 过期时间那两行）。
  final VoidCallback? onTap;

  /// 选中态描边覆盖。null = accentEdge（可勾选行，React `:282` 的
  /// `--accent-edge`）；只读展示行传 accentText（React `:257/:362` 的
  /// `--accent`，mono.ts:70 映射 accent-text）。
  final Color? selectedBorder;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.sxs),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(AidogRadius.sm),
        child: Container(
          // `SmartPasteModal.tsx:175`：padding 7px 10px（竖 7 / 横 10）。
          padding: const EdgeInsets.symmetric(
            horizontal: AidogSpace.smd,
            vertical: 7,
          ),
          decoration: BoxDecoration(
            color: theme.c.surface2,
            border: Border.all(
              color: selected
                  ? (selectedBorder ?? theme.c.accentEdge)
                  : theme.c.line,
            ),
            borderRadius: BorderRadius.circular(AidogRadius.sm),
          ),
          child: Row(
            children: [
              if (leading != null) ...[
                leading!,
                const SizedBox(width: AidogSpace.sxs),
              ],
              Expanded(child: child),
            ],
          ),
        ),
      ),
    );
  }
}
