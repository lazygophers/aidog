/// 模型测试面板的逻辑层（票 I09），对应 `src/pages/ModelTestPanel.tsx`。
///
/// 六种测试模式决定「测哪些模型」，然后**串行**逐个发 `model_test`，每测完一条就把
/// 结果推进列表（React 的 `setResults([...res])`）—— 不是等全部跑完再一次性渲染。
library;

import 'package:flutter/foundation.dart' show VoidCallback;

import 'invoke.dart';
import 'models.dart' show ModelTestResult;

export 'models.dart' show ModelTestResult;

/// `ModelTestPanel.tsx:22::TestMode`，顺序 = 按钮顺序（`:110` 的 modes 数组）。
const List<String> kTestModes = [
  'quick',
  'single',
  'batch',
  'random',
  'custom',
  'tool',
];

/// 被测平台的最小投影（`Platform` 里这面板只用到这几样）。
class TestTargetPlatform {
  const TestTargetPlatform({
    required this.id,
    required this.name,
    required this.platformType,
    this.availableModels = const [],
    this.models = const {},
  });

  final int id;
  final String name;
  final String platformType;

  /// `platform.available_models`。
  final List<String> availableModels;

  /// `platform.models`：default / sonnet / opus / haiku / gpt。
  final Map<String, String> models;

  /// `ModelTestPanel.tsx:35`：有 available_models 就用它；
  /// 否则按 **default / sonnet / opus / haiku / gpt** 的顺序取非空值。
  List<String> get allModels {
    if (availableModels.isNotEmpty) return availableModels;
    return [
      for (final k in const ['default', 'sonnet', 'opus', 'haiku', 'gpt'])
        if ((models[k] ?? '').isNotEmpty) models[k]!,
    ];
  }

  /// `ModelTestPanel.tsx:39`：default 非空取它，否则取列表第一条，都没有给空串。
  String get defaultModel {
    final d = models['default'] ?? '';
    if (d.isNotEmpty) return d;
    final all = allModels;
    return all.isNotEmpty ? all.first : '';
  }
}

class ModelTestController {
  ModelTestController({
    required this.invoke,
    required this.onChanged,
    required this.platform,
    this.onResult,
  });

  final InvokeFn invoke;
  final VoidCallback onChanged;
  final TestTargetPlatform platform;

  /// 整轮结束后回调「是不是全绿」（React 的 `onResult`）。
  final void Function(bool allOk)? onResult;

  String mode = 'quick';
  List<String> selectedModels = const [];
  String customPrompt = '';
  List<ModelTestResult> results = const [];
  bool running = false;

  /// 正在测第几条（-1 = 不在测）。
  int currentIdx = -1;

  /// `ModelTestPanel.tsx:41::getModels`，六个分支一字不改。
  List<String> get models => switch (mode) {
    'quick' => [platform.defaultModel],
    'single' =>
      selectedModels.isNotEmpty
          ? [selectedModels.first]
          : [platform.defaultModel],
    'batch' =>
      selectedModels.isNotEmpty
          ? selectedModels
          : platform.allModels.take(5).toList(),
    'random' => platform.allModels,
    'custom' =>
      selectedModels.isNotEmpty
          ? [selectedModels.first]
          : [platform.defaultModel],
    // 工具调用探测：对全部模型发起 get_weather 工具测试。
    'tool' => platform.allModels,
    _ => const [],
  };

  /// `ModelTestPanel.tsx:119`。
  bool get needsModelSelect =>
      mode == 'single' || mode == 'batch' || mode == 'custom';

  /// `ModelTestPanel.tsx:190` 的 disabled 表达式，照抄（注意 `mode !== "batch"` 那一段）。
  bool get runDisabled =>
      running ||
      (needsModelSelect &&
          selectedModels.isEmpty &&
          mode != 'batch' &&
          platform.allModels.isEmpty);

  /// 切模式**同时清空选中模型与结果**（`ModelTestPanel.tsx:162`）。
  void setMode(String m) {
    mode = m;
    selectedModels = const [];
    results = const [];
    onChanged();
  }

  void toggleModel(String m) {
    selectedModels = selectedModels.contains(m)
        ? [
            for (final x in selectedModels)
              if (x != m) x,
          ]
        : [...selectedModels, m];
    onChanged();
  }

  void setCustomPrompt(String p) {
    customPrompt = p;
    onChanged();
  }

  /// `ModelTestPanel.tsx:71::runTest`。空清单直接返回（连 running 都不置）。
  Future<void> run() async {
    final list = models;
    if (list.isEmpty) return;
    running = true;
    results = const [];
    onChanged();

    final res = <ModelTestResult>[];
    for (var i = 0; i < list.length; i++) {
      currentIdx = i;
      onChanged();
      try {
        final raw = await invoke('model_test', {
          'req': {
            'platform_id': platform.id,
            // `prompt` 只在 custom 模式且非空时带上；`tool_test` 只在 tool 模式带 true。
            if (mode == 'custom' && customPrompt.isNotEmpty)
              'prompt': customPrompt,
            if (mode == 'tool') 'tool_test': true,
            'model': list[i],
          },
        });
        res.add(ModelTestResult.fromJson((raw as Map).cast<String, dynamic>()));
      } catch (e) {
        res.add(ModelTestResult.failure(list[i], e));
      }
      results = [...res];
      onChanged();
    }
    running = false;
    currentIdx = -1;
    onChanged();
    // 整轮全绿才算成功（空结果不算成功）。
    onResult?.call(res.isNotEmpty && res.every((r) => r.success));
  }
}
