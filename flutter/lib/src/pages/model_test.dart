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
      child: Tile(
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
                  SmallButton(
                    label: t.t('test.mode${_modeSuffix(m)}'),
                    active: _c.mode == m,
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
                    SmallButton(
                      label: m,
                      active: _c.selectedModels.contains(m),
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
                style: AidogType.micro.copyWith(color: theme.c.fg),
                onChanged: _c.setCustomPrompt,
              ),
            ],
            const SizedBox(height: AidogSpace.ssm),
            Row(
              children: [
                SmallButton(
                  label: _c.running
                      ? '${t.t('test.running')}'
                            '${_c.currentIdx >= 0 ? ltr(' (${_c.currentIdx + 1}/${_c.models.length})') : ''}'
                      : t.t('test.run'),
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
              for (final r in _c.results)
                Padding(
                  padding: const EdgeInsets.only(top: AidogSpace.sxs),
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
                              style: AidogType.micro.copyWith(
                                color: theme.c.fg,
                              ),
                            ),
                          ),
                          // 零值不渲染（React 的 `> 0 &&`）：没测到就别画一个假的 0ms。
                          if (r.durationMs > 0)
                            Text(
                              ltr('${r.durationMs}ms'),
                              style: AidogType.micro.copyWith(
                                color: theme.c.fg3,
                              ),
                            ),
                          if (r.outputTokens > 0)
                            Padding(
                              padding: const EdgeInsets.only(
                                left: AidogSpace.sxs,
                              ),
                              child: Text(
                                ltr('${r.inputTokens + r.outputTokens} tok'),
                                style: AidogType.micro.copyWith(
                                  color: theme.c.fg3,
                                ),
                              ),
                            ),
                        ],
                      ),
                      if (r.error.isNotEmpty)
                        Text(
                          r.error,
                          style: AidogType.micro.copyWith(color: theme.c.bad),
                        ),
                      if (r.responsePreview.isNotEmpty)
                        Text(
                          r.responsePreview,
                          style: AidogType.micro.copyWith(color: theme.c.fg2),
                        ),
                    ],
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
