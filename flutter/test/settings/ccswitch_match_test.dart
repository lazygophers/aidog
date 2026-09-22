/// cc-switch provider → Platform JSON 的匹配回退链与转换
/// （`lib/src/pages/settings/ccswitch_match.dart`，`src/utils/ccswitchMatch.ts` 的 Dart 版）。
///
/// 回归的是这条：Flutter 侧原先把 cc-switch 的**原始 provider map** 直接当
/// `platformPayload` 发给后端（`importexport_page.dart:718` 的 `(chosen) => chosen`），
/// 而后端收的是已经转好的 Platform JSON，缺字段一律写空串 ——
/// 导进去的是一串没有协议、没有 base_url、没有密钥的空壳平台。
library;

import 'package:aidog_flutter/src/pages/platform_card_bits.dart'
    show ProtocolMetaTable;
import 'package:aidog_flutter/src/pages/platform_defaults.dart';
import 'package:aidog_flutter/src/pages/settings/ccswitch_match.dart';
import 'package:flutter_test/flutter_test.dart';

/// 两份元数据都来自同一份 registry 文档 —— 与页面上的取法一致。
/// glm 带关键词，anthropic 只靠 host（故意不给关键词，用来验步骤 2）。
const String _defaultsJson = '''
{"protocols":{
  "glm":{"name":{"en-US":"Zhipu GLM"},"keywords":["glm","zhipu"],
    "endpoints":{"default":[{"protocol":"glm","base_url":"https://open.bigmodel.cn/api/paas/v4"}]}},
  "anthropic":{"name":{"en-US":"Anthropic"},
    "endpoints":{"default":[{"protocol":"anthropic","base_url":"https://api.anthropic.com"}]}},
  "openai":{"name":{"en-US":"OpenAI"},
    "endpoints":{"default":[{"protocol":"openai","base_url":"https://api.openai.com/v1"}]}}
}}''';

Map<String, Object?> _claude({
  String name = 'x',
  String? baseUrl,
  String? apiKey,
  Map<String, Object?>? env,
}) => {
  'id': 'p1',
  'appType': 'claude',
  'name': name,
  'settingsConfig': {'env': env ?? const <String, Object?>{}},
  'detectedBaseUrl': baseUrl,
  'detectedApiKey': apiKey,
};

Map<String, Object?> _codex({
  String name = 'x',
  String? baseUrl,
  String? wireApi,
  String? model,
}) => {
  'id': 'p2',
  'appType': 'codex',
  'name': name,
  'settingsConfig': const <String, Object?>{},
  'detectedBaseUrl': baseUrl,
  'codexConfigParsed': {'wireApi': wireApi, 'model': model},
};

void main() {
  final meta = ProtocolMetaTable.parse(_defaultsJson, 'en-US');
  final defaults = PlatformDefaults.parse(_defaultsJson, '', 'en-US');

  group('匹配回退链（ccswitchMatch.ts:163）', () {
    test('步骤 1：名字命中 preset 关键词', () {
      final m = matchCcProvider(
        _claude(name: 'my GLM account'),
        meta: meta,
        defaults: defaults,
      );
      expect(m.protocol, 'glm');
      expect(m.matchedBy, CcMatchedBy.presetKeyword);
      expect(m.matchedLabel, 'Zhipu GLM');
    });

    test('步骤 1：名字没线索但 base_url 里有关键词也算命中（name + url 一起当 hay）', () {
      final m = matchCcProvider(
        _claude(name: '账号一', baseUrl: 'https://zhipu.example.com/v1'),
        meta: meta,
        defaults: defaults,
      );
      expect(m.protocol, 'glm');
      expect(m.matchedBy, CcMatchedBy.presetKeyword);
    });

    test('步骤 2：关键词没命中 → 按 base_url 的 host 匹配', () {
      final m = matchCcProvider(
        _claude(name: '账号二', baseUrl: 'https://api.anthropic.com'),
        meta: meta,
        defaults: defaults,
      );
      expect(m.protocol, 'anthropic');
      expect(m.matchedBy, CcMatchedBy.baseUrlHost);
    });

    test('命中之后：preset 骨架端点里同协议那条换成实际 base_url', () {
      const url = 'https://relay.example.com/api/paas/v4';
      final m = matchCcProvider(
        _claude(name: 'glm 中转', baseUrl: url),
        meta: meta,
        defaults: defaults,
      );
      expect(m.endpoints.single.protocol, 'glm');
      expect(m.endpoints.single.baseUrl, url);
      expect(m.baseUrl, url);
    });

    test('步骤 3：codex + wireApi=responses → openai_responses 端点，客户端按 codex 模拟', () {
      final m = matchCcProvider(
        _codex(name: '某中转', baseUrl: 'https://nowhere.test/v1', wireApi: 'responses'),
        meta: meta,
        defaults: defaults,
      );
      expect(m.protocol, 'openai');
      expect(m.matchedBy, CcMatchedBy.protocolFallback);
      expect(m.endpoints.single.protocol, 'openai_responses');
      expect(m.endpoints.single.clientType, 'codex_tui');
    });

    test('步骤 3：codex 没写 wireApi → 退到 openai 端点', () {
      final m = matchCcProvider(
        _codex(name: '某中转', baseUrl: 'https://nowhere.test/v1'),
        meta: meta,
        defaults: defaults,
      );
      expect(m.endpoints.single.protocol, 'openai');
    });

    test('步骤 4：claude 一路没命中 → anthropic，即使 URL 看着像 openai', () {
      final m = matchCcProvider(
        _claude(name: '某中转', baseUrl: 'https://nowhere.test/v1/chat/completions'),
        meta: meta,
        defaults: defaults,
      );
      expect(m.protocol, 'anthropic');
      expect(m.matchedBy, CcMatchedBy.protocolFallback);
      expect(m.endpoints.single.clientType, 'claude_code');
    });

    test('registry 没到手（空表）也不崩，退到协议回退那一支', () {
      final m = matchCcProvider(
        _claude(name: 'my GLM account', baseUrl: 'https://open.bigmodel.cn'),
        meta: const ProtocolMetaTable(),
        defaults: PlatformDefaults.empty,
      );
      expect(m.protocol, 'anthropic');
      expect(m.matchedBy, CcMatchedBy.protocolFallback);
    });
  });

  group('模型映射抽取（ccswitchMatch.ts:200）', () {
    test('claude：四个 env 各进各的槽位', () {
      final m = extractCcModels(
        _claude(
          env: {
            'ANTHROPIC_MODEL': 'claude-x',
            'ANTHROPIC_DEFAULT_HAIKU_MODEL': 'h',
            'ANTHROPIC_DEFAULT_SONNET_MODEL': 's',
            'ANTHROPIC_DEFAULT_OPUS_MODEL': 'o',
          },
        ),
      );
      expect(m.defaultModel, 'claude-x');
      expect(m.haiku, 'h');
      expect(m.sonnet, 's');
      expect(m.opus, 'o');
    });

    test('空串按「没有」处理，不写成空字符串槽位', () {
      final m = extractCcModels(_claude(env: {'ANTHROPIC_MODEL': ''}));
      expect(m.defaultModel, isNull);
    });

    test('codex：取 config.toml 的 model', () {
      expect(extractCcModels(_codex(model: 'gpt-x')).defaultModel, 'gpt-x');
    });
  });

  group('转成 Platform JSON（ccswitchMatch.ts:236）', () {
    Map<String, Object?> convert(
      Map<String, Object?> p, {
      CcImportDims dims = const CcImportDims(),
    }) => ccProviderToPlatformJson(
      p,
      matchCcProvider(p, meta: meta, defaults: defaults),
      dims,
    );

    test('协议 / base_url / 密钥 / 端点都落进去了（这就是「空壳平台」的反面）', () {
      final json = convert(
        _claude(
          name: 'GLM 主号',
          baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
          apiKey: 'sk-test',
        ),
      );
      expect(json['name'], 'GLM 主号');
      expect(json['platform_type'], 'glm');
      expect(json['base_url'], 'https://open.bigmodel.cn/api/paas/v4');
      expect(json['api_key'], 'sk-test');
      expect((json['endpoints']! as List), isNotEmpty);
      expect(json['enabled'], true);
      expect(json['status'], 'enabled');
    });

    test('关掉密钥那一维 → api_key 是空串，其余照旧', () {
      final json = convert(
        _claude(name: 'GLM', apiKey: 'sk-test'),
        dims: const CcImportDims(d4: false),
      );
      expect(json['api_key'], '');
      expect(json['platform_type'], 'glm');
    });

    test('关掉模型那一维 → models 是空表', () {
      final json = convert(
        _claude(name: 'GLM', env: {'ANTHROPIC_MODEL': 'claude-x'}),
        dims: const CcImportDims(d2: false),
      );
      expect(json['models'], isEmpty);
    });

    test('开着模型那一维 → models 带上 default 槽', () {
      final json = convert(
        _claude(name: 'GLM', env: {'ANTHROPIC_MODEL': 'claude-x'}),
      );
      expect((json['models']! as Map)['default'], 'claude-x');
    });
  });
}
