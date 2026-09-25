/// 模型测试面板（票 I09），对应 `src/pages/ModelTestPanel.tsx`。
///
/// React 那边是平台卡点「测试」弹出的 Radix Dialog；这里与票 I07 的确认卡同一做法 ——
/// 做成一块页面内的格子，`find.byType(ModelTestPanel)` 就能断言。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';
import 'invoke.dart';
import 'model_test_logic.dart';
import 'ui_bits.dart';

class ModelTestPanel extends StatefulWidget {
  const ModelTestPanel({
    super.key,
    this.invoke = kernelInvoke,
    required this.platform,
    required this.onClose,
    this.onResult,
  });

  final InvokeFn invoke;
  final TestTargetPlatform platform;
  final VoidCallback onClose;
  final void Function(bool allOk)? onResult;

  @override
  State<ModelTestPanel> createState() => _ModelTestPanelState();
}

class _ModelTestPanelState extends State<ModelTestPanel> {
  late final ModelTestController _c;

  @override
  void initState() {
    super.initState();
    _c = ModelTestController(
      invoke: widget.invoke,
      onChanged: () {
        if (mounted) setState(() {});
      },
      platform: widget.platform,
      onResult: widget.onResult,
    );
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    // React 侧是普通 `Dialog`（`ModelTestPanel.tsx:145`，width 560）。跑测试中
    // 关闭按钮本就禁用，遮罩同步不可关。
    return AidogModal(
      maxWidth: 560,
      onBarrierTap: _c.running ? null : widget.onClose,
      child: ModalCard(
        // `ModelTestPanel.tsx:146-150`：radius 16（非 glass-elevated 的 24）、
        // 标题 15 w700（非 DialogTitle 默认 17 w600）。
        radius: 16,
        titleStyle: AidogType.title.copyWith(
          fontSize: 15,
          fontWeight: FontWeight.w700,
        ),
        title: t.t('test.title'),
        meta: '${widget.platform.name} · ${widget.platform.platformType}',
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Wrap(
              spacing: AidogSpace.sxs,
              runSpacing: AidogSpace.sxs,
              children: [
                for (final m in kTestModes)
                  // `ModelTestPanel.tsx:161`：11、pad 4/8。
                  SmallButton(
                    label: t.t('test.mode${_modeSuffix(m)}'),
                    active: _c.mode == m,
                    fontSize: 11,
                    padding: (8, 4),
                    onTap: () => _c.setMode(m),
                  ),
              ],
            ),
            // 三种模式要选模型（quick / random / tool 自己决定测哪些）。
            if (_c.needsModelSelect) ...[
              const SizedBox(height: AidogSpace.ssm),
              Wrap(
                spacing: AidogSpace.sxs,
                runSpacing: AidogSpace.sxs,
                children: [
                  for (final m in widget.platform.allModels)
                    // `ModelTestPanel.tsx:173`：11、pad 3/8。
                    SmallButton(
                      label: m,
                      active: _c.selectedModels.contains(m),
                      fontSize: 11,
                      padding: (8, 3),
                      onTap: () => _c.toggleModel(m),
                    ),
                ],
              ),
            ],
            if (_c.mode == 'custom') ...[
              const SizedBox(height: AidogSpace.ssm),
              TextField(
                key: const Key('model-test-prompt'),
                maxLines: 3,
                decoration: InputDecoration(
                  isDense: true,
                  hintText: t.t('test.promptPlaceholder'),
                ),
                style: AidogType.label.copyWith(color: theme.c.fg),
                onChanged: _c.setCustomPrompt,
              ),
            ],
            const SizedBox(height: AidogSpace.ssm),
            Row(
              children: [
                // `ModelTestPanel.tsx:188`：13、pad 8/16，默认变体 = 实心。
                SmallButton(
                  label: _c.running
                      ? '${t.t('test.running')}'
                            '${_c.currentIdx >= 0 ? ltr(' (${_c.currentIdx + 1}/${_c.models.length})') : ''}'
                      : t.t('test.run'),
                  filled: true,
                  active: true,
                  fontSize: 13,
                  padding: (16, 8),
                  onTap: _c.runDisabled ? null : _c.run,
                ),
                const Spacer(),
                SmallButton(
                  label: t.t('action.close'),
                  onTap: _c.running ? null : widget.onClose,
                ),
              ],
            ),
            if (_c.results.isNotEmpty) ...[
              const SizedBox(height: AidogSpace.ssm),
              TileMeta(t.t('test.results')),
              for (final (i, r) in _c.results.indexed)
                // `ModelTestPanel.tsx:122-141`：glass-surface 行卡 pad 10/14 +
                // 左缘 3px 成败色 + reveal stagger 60。
                Reveal(
                  delayMs: i * 60,
                  child: Padding(
                    // React 结果列表 gap 6（`ModelTestPanel.tsx:198`）。
                    padding: const EdgeInsets.only(bottom: AidogSpace.ssm),
                    // 左缘 3px 成败色：与通知卡同一条路（notifications.dart:166，
                    // 非 uniform Border 配不了圆角，竖条画在卡外）。
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        border: BorderDirectional(
                          start: BorderSide(
                            width: 3,
                            color: r.success ? theme.c.ok : theme.c.bad,
                          ),
                        ),
                      ),
                      child: Tile(
                        // `ModelTestPanel.tsx:126`：pad 10/14（竖/横）。
                        padding: const EdgeInsets.symmetric(
                          horizontal: AidogSpace.slg,
                          vertical: AidogSpace.smd,
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Row(
                              children: [
                                Icon(
                                  r.success ? Icons.check_circle : Icons.error,
                                  size: 13,
                                  color: r.success ? theme.c.ok : theme.c.bad,
                                ),
                                const SizedBox(width: AidogSpace.sxs),
                                Expanded(
                                  child: Text(
                                    r.model,
                                    // `ModelTestPanel.tsx:130`：12 w600。
                                    style: AidogType.caption.copyWith(
                                      fontSize: 12,
                                      fontWeight: FontWeight.w600,
                                      color: theme.c.fg,
                                    ),
                                  ),
                                ),
                                // 零值不渲染（React 的 `> 0 &&`）：没测到就别画一个假的 0ms。
                                // 元信息 11 fg2（`:131`）。
                                if (r.durationMs > 0)
                                  Text(
                                    ltr('${r.durationMs}ms'),
                                    style: AidogType.micro.copyWith(
                                      color: theme.c.fg2,
                                    ),
                                  ),
                                if (r.outputTokens > 0)
                                  Padding(
                                    padding: const EdgeInsets.only(
                                      left: AidogSpace.ssm,
                                    ),
                                    child: Text(
                                      ltr(
                                        '${r.inputTokens + r.outputTokens} tok',
                                      ),
                                      style: AidogType.micro.copyWith(
                                        color: theme.c.fg2,
                                      ),
                                    ),
                                  ),
                              ],
                            ),
                            if (r.error.isNotEmpty)
                              Padding(
                                padding: const EdgeInsets.only(
                                  top: AidogSpace.sxs,
                                ),
                                child: Text(
                                  r.error,
                                  style: AidogType.micro.copyWith(
                                    color: theme.c.bad,
                                  ),
                                ),
                              ),
                            if (r.responsePreview.isNotEmpty)
                              Text(
                                r.responsePreview,
                                style: AidogType.micro.copyWith(
                                  color: theme.c.fg2,
                                ),
                              ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
            ],
          ],
        ),
      ),
    );
  }

  /// `test.modeQuick` / `modeSingle` / … 的键名后缀。
  static String _modeSuffix(String mode) =>
      mode[0].toUpperCase() + mode.substring(1);
}
