/// 智能识别解析器的单测（票 20），逐条翻译自 `src/utils/platformPaste.test.ts`，
/// **fixture 数据与期望值一字不改**。
///
/// 没翻的只有 React 那边最后一段「全协议回归矩阵」：它直接从
/// `src-tauri/defaults/registry/` 读全部 platform.json 做数据驱动断言。Dart 侧再读一遍
/// 同一批文件只会得到同一个结论，却把纯逻辑单测绑死在仓库目录结构上；那段回归由 React
/// 侧的 `platformPaste.test.ts:647` 守着，本文件守的是移植本身的等价性。
library;

import 'dart:convert';

import 'package:aidog_flutter/pages.dart';
import 'package:flutter_test/flutter_test.dart';

/// `platformPaste.test.ts:18::PRESETS`，逐字照抄。
const List<PastePresetRef> presets = [
  PastePresetRef(
    value: 'anthropic',
    label: 'Anthropic',
    keywords: ['claude', '克劳德', '官方'],
    hosts: ['api.anthropic.com'],
    keyPrefixes: ['sk-ant-'],
  ),
  PastePresetRef(
    value: 'openai',
    label: 'OpenAI',
    keywords: ['gpt', 'chatgpt', '官方'],
    hosts: ['api.openai.com/v1'],
    keyPrefixes: ['sk-proj-'],
  ),
  PastePresetRef(
    value: 'deepseek',
    label: 'DeepSeek',
    keywords: ['deepseek'],
    hosts: ['api.deepseek.com'],
  ),
  PastePresetRef(
    value: 'glm',
    label: 'GLM',
    keywords: ['glm', '智谱'],
    hosts: ['open.bigmodel.cn/api/paas/v4'],
  ),
  PastePresetRef(
    value: 'glm_coding',
    label: 'GLM Coding',
    keywords: ['glm coding'],
    hosts: ['open.bigmodel.cn/api/coding'],
    codingPlan: true,
  ),
  PastePresetRef(
    value: 'xiaomi_mimo',
    label: 'Xiaomi MiMo',
    keywords: ['xiaomi', 'mimo'],
    hosts: ['api.xiaomimimo.com'],
  ),
  PastePresetRef(
    value: 'xiaomi_mimo_coding',
    label: 'Xiaomi MiMo Coding',
    hosts: ['token-plan-cn.xiaomimimo.com'],
    codingPlan: true,
    keyPrefixes: ['tp-'],
  ),
  PastePresetRef(
    value: 'doubao',
    label: '火山引擎',
    keywords: ['火山', 'doubao', 'volces', 'agentplan'],
    keyPrefixes: ['ark-'],
    hosts: [
      'ark.cn-beijing.volces.com/api/coding',
      'ark.cn-beijing.volces.com/api/coding/v3',
      'ark.cn-beijing.volces.com/api/plan',
      'ark.cn-beijing.volces.com/api/plan/v3',
    ],
  ),
  PastePresetRef(value: 'mock', label: 'Mock', keywords: ['测试', 'mock']),
  // paste_fallback 兜底平台（registry newapi）。
  const PastePresetRef(
    value: 'newapi',
    label: 'New API',
    keywords: ['newapi', 'new-api', '中转'],
    pasteFallback: true,
  ),
];

String b64(String s) => base64.encode(utf8.encode(s));

List<String> urlsOf(ParsedPaste p) => [for (final b in p.baseUrls) b.url];

void main() {
  group('normalizeForMatch', () {
    test('小写化、非 alnum/CJK 换空格、折叠空白', () {
      expect(normalizeForMatch('Hello,  WORLD!'), 'hello world');
      expect(normalizeForMatch('  GLM-4.5  '), 'glm 4 5');
    });
    test('保留 CJK', () {
      expect(normalizeForMatch('智谱AI'), '智谱ai');
    });
  });

  group('guessProtocol', () {
    test('anthropic（容忍截断）', () {
      expect(
        guessProtocol('https://api.anthropic.com'),
        ParsedProtocol.anthropic,
      );
      expect(guessProtocol('https://x.com/anthropi'), ParsedProtocol.anthropic);
    });
    test('gemini', () {
      expect(
        guessProtocol('https://generativelanguage.googleapis.com'),
        ParsedProtocol.gemini,
      );
      expect(guessProtocol('https://x/gemini'), ParsedProtocol.gemini);
    });
    test('openai（含 /v1 路径）', () {
      expect(guessProtocol('https://api.openai.com'), ParsedProtocol.openai);
      expect(guessProtocol('https://host/v1'), ParsedProtocol.openai);
    });
    test('认不出就 unknown', () {
      expect(guessProtocol('https://example.com/foo'), ParsedProtocol.unknown);
    });
  });

  group('matchPlatform', () {
    test('paste_fallback：未知 host 的 API URL 命中 newapi，压过模型名词噪', () {
      final hit = matchPlatform('Qwen3.8-Flash-Next 临时福利', presets, const [
        ParsedBaseUrl('https://api.lasong.xyz/v1', ParsedProtocol.openai),
      ]);
      expect(hit?.value, 'newapi');
    });

    test('paste_fallback：非 API URL（unknown 协议）不触发，keyword 扫描照常', () {
      final hit = matchPlatform('使用 deepseek 模型', presets, const [
        ParsedBaseUrl('https://github.com/foo/bar', ParsedProtocol.unknown),
      ]);
      expect(hit?.value, 'deepseek');
    });

    test('按 base_url host 匹配，最长最特异者胜出', () {
      final hit = matchPlatform('', presets, const [
        ParsedBaseUrl(
          'https://token-plan-cn.xiaomimimo.com/v1',
          ParsedProtocol.openai,
        ),
      ]);
      expect(hit?.value, 'xiaomi_mimo_coding');
      expect(hit?.codingPlan, isTrue);
    });

    test('同 host 的 coding 与普通版靠 path 子串区分', () {
      final coding = matchPlatform('', presets, const [
        ParsedBaseUrl(
          'https://open.bigmodel.cn/api/coding/paas/v4',
          ParsedProtocol.openai,
        ),
      ]);
      expect(coding?.value, 'glm_coding');
      final normal = matchPlatform('', presets, const [
        ParsedBaseUrl(
          'https://open.bigmodel.cn/api/paas/v4',
          ParsedProtocol.openai,
        ),
      ]);
      expect(normal?.value, 'glm');
    });

    test('doubao：粘 /api/coding/v3 与 /api/coding 都命中 doubao', () {
      final v3 = matchPlatform('', presets, const [
        ParsedBaseUrl(
          'https://ark.cn-beijing.volces.com/api/coding/v3',
          ParsedProtocol.openai,
        ),
      ]);
      expect(v3?.value, 'doubao');
      final plain = matchPlatform('', presets, const [
        ParsedBaseUrl(
          'https://ark.cn-beijing.volces.com/api/coding',
          ParsedProtocol.anthropic,
        ),
      ]);
      expect(plain?.value, 'doubao');
    });

    test('没有 host 命中时回落 keyword 扫描', () {
      expect(matchPlatform('使用 deepseek 模型', presets)?.value, 'deepseek');
    });

    test('mock 即便 keyword 命中也被排除', () {
      expect(matchPlatform('跑个测试', presets), isNull);
    });

    test('什么都不命中返回 null', () {
      expect(matchPlatform('random gibberish text', presets), isNull);
    });
  });

  group('parsePlatformPaste', () {
    test('空文本 → 空结果', () {
      final out = parsePlatformPaste('', presets);
      expect(out.apiKeys, isEmpty);
      expect(out.baseUrls, isEmpty);
      expect(out.platform, isNull);
      expect(out.models, isEmpty);
      expect(out.expiresAt, isNull);
      expect(parsePlatformPaste('   ', presets).platform, isNull);
    });

    test('抽前缀锚定的 api key', () {
      final out = parsePlatformPaste(
        'key: sk-ant-abcdefghijklmnop1234 base https://api.anthropic.com',
        presets,
      );
      expect(out.apiKeys.any((k) => k.startsWith('sk-ant-')), isTrue);
      expect(out.baseUrls.any((b) => b.url.contains('anthropic')), isTrue);
    });

    test('剔掉插进 key 里的防爬汉字', () {
      final out = parsePlatformPaste(
        'apikey: sk-请删除这些字proj1234567890abcd',
        presets,
      );
      expect(out.apiKeys, isNotEmpty);
      expect(out.apiKeys.first, isNot(matches(RegExp('[一-鿿]'))));
    });

    test('赋值锚定里的 base64 key 会被解码', () {
      final enc = b64('sk-decoded-key-1234567890');
      final out = parsePlatformPaste('API_KEY（base64编码）: $enc', presets);
      expect(out.apiKeys, contains('sk-decoded-key-1234567890'));
    });

    test('抽 base_url、去重、跳过图片资源', () {
      final out = parsePlatformPaste(
        'url https://api.deepseek.com/v1 logo https://cdn.x/logo.png '
        'again https://api.deepseek.com/v1',
        presets,
      );
      final urls = urlsOf(out);
      expect(urls, contains('https://api.deepseek.com/v1'));
      expect(urls.any((u) => u.endsWith('.png')), isFalse);
      expect(urls.where((u) => u == 'https://api.deepseek.com/v1').length, 1);
    });

    test('从抽出的 base_url 匹配平台', () {
      final out = parsePlatformPaste(
        '接口地址 https://api.deepseek.com/v1',
        presets,
      );
      expect(out.platform?.value, 'deepseek');
    });

    test('裸 base64（无标签）解码后是键形就采纳', () {
      final enc = b64('tt-barekeyabcdefghij1234567890');
      final out = parsePlatformPaste('随便一段文字 $enc 结尾', presets);
      expect(out.apiKeys, contains('tt-barekeyabcdefghij1234567890'));
    });

    test('防爬汉字切断的裸 base64 会被拼回再解码', () {
      final full = b64('sk-joinedkeyabcdefghij1234567890');
      final cut = full.length ~/ 2;
      final injected =
          '${full.substring(0, cut)}删掉我再base64解码${full.substring(cut)}';
      final out = parsePlatformPaste(injected, presets);
      expect(out.apiKeys.any((k) => k.startsWith('sk-joinedkey')), isTrue);
    });

    test('赋值锚定的明文长 key（无已知前缀）照样留下', () {
      final out = parsePlatformPaste(
        'token: ABCDEFGHIJKLMNOPQRSTUVWX1234',
        presets,
      );
      expect(out.apiKeys, contains('ABCDEFGHIJKLMNOPQRSTUVWX1234'));
    });

    test('base64 解码后的中文标签复合串', () {
      final enc = b64(
        '令牌sk-cmp-abcdefghijklmnop地址https://api.deepseek.com/v1模型deepseek-chat',
      );
      final out = parsePlatformPaste(enc, presets);
      expect(out.apiKeys.any((k) => k.startsWith('sk-cmp-')), isTrue);
      expect(out.baseUrls.any((b) => b.url.contains('deepseek')), isTrue);
      expect(out.models, isNotEmpty);
    });

    test('MiMo token plan：防爬 base64 里的 tp- key → coding 平台', () {
      final full = b64('tp-cd0mfe829kk20chvj4n92ujw8synkxw5vqv5z67qx2k569qv');
      final cut = full.length ~/ 2;
      final injected =
          '分享MIMO 即将过期 ${full.substring(0, cut)}使劲蹬啊${full.substring(cut)} '
          '自己蹬不动了 lark_024';
      final out = parsePlatformPaste(injected, presets);
      expect(out.apiKeys.any((k) => k.startsWith('tp-')), isTrue);
      expect(out.platform?.value, 'xiaomi_mimo_coding');
      expect(out.platform?.codingPlan, isTrue);
    });

    test('整段 base64 分享文本：裸 key 补提 + MiMo coding + /v1 base_url', () {
      // `platformPaste.test.ts:264` 的同一份真实样本。
      const enc =
          '5YW85a65IE9wZW5BSSDmjqXlj6PljY/orq7vvJoKaHR0cHM6Ly90b2tlbi1wbGFuLWNuLnhpYW9taW1pbW8uY29tL3YxCuWFvOWuuSBBbnRocm9waWMg5o6l5Y+j5Y2P6K6u77yaCmh0dHBzOi8vdG9rZW4tcGxhbi1jbi54aWFvbWltaW1vLmNvbS9hbnRocm9waWMKdHAtY3R6Ymg2ODF1NmRnYzVheHJ6czdycm5mYWpjaDkydzA2cTgweXI2ODA3NXdoNjQ3';
      final out = parsePlatformPaste(enc, presets);
      expect(out.apiKeys.any((k) => k.startsWith('tp-ctzbh')), isTrue);
      expect(out.platform?.value, 'xiaomi_mimo_coding');
      expect(out.platform?.codingPlan, isTrue);
      expect(
        out.baseUrls.any(
          (b) => b.url == 'https://token-plan-cn.xiaomimimo.com/v1',
        ),
        isTrue,
      );
    });

    test('优先级 2：纯 token 粘贴命中 key_prefixes → 直判 coding 平台', () {
      final out = parsePlatformPaste(
        '小米 MiMo 套餐 key: tp-abc1234567890defghijklmnop',
        presets,
      );
      expect(out.apiKeys.any((k) => k.startsWith('tp-')), isTrue);
      expect(out.platform?.value, 'xiaomi_mimo_coding');
      expect(out.platform?.codingPlan, isTrue);
    });

    test('优先级 2 守卫：普通 mimo key 不误判成 coding', () {
      final out = parsePlatformPaste(
        '小米 MiMo 普通版 key: sk-abc1234567890defghijklmnop',
        presets,
      );
      expect(out.platform?.value, 'xiaomi_mimo');
      expect(out.platform?.codingPlan, isFalse);
    });

    test('MiMo PRO 分享文案：coding plan + expiresAt 联合识别', () {
      final out = parsePlatformPaste(
        'MiMo PRO 分享 https://token-plan-cn.xiaomimimo.com/v1 '
        'key tp-abc1234567890defghij 6.27 到期',
        presets,
      );
      expect(out.platform?.value, 'xiaomi_mimo_coding');
      expect(out.platform?.codingPlan, isTrue);
      expect(out.expiresAt, isNotNull);
      expect(out.expiresAt! > 0, isTrue);
    });

    test('lark 里的 ark 子串不误判成 doubao', () {
      final out = parsePlatformPaste(
        '由 lin2101 发布 lark_024 文化宣导员 sgp吗',
        presets,
      );
      expect(out.platform?.value, isNot('doubao'));
    });

    test('ark- 前缀 key 抽得出来', () {
      final out = parsePlatformPaste(
        '火山 key: ark-9a96aed4c0e474c9c0949581a00fef7c3c6',
        presets,
      );
      expect(out.apiKeys.any((k) => k.startsWith('ark-')), isTrue);
      expect(out.apiKeys.any((k) => k.contains('9a96aed')), isTrue);
    });

    test('圈数字防爬字符（①-⓿）从 key 里剔干净', () {
      final out = parsePlatformPaste(
        'key: ark-9a②96aed-4c0e-474c-9c09-49⑤8⑨1a00fef-7c3c6',
        presets,
      );
      expect(out.apiKeys, isNotEmpty);
      expect(out.apiKeys.first, isNot(matches(RegExp('[①-⓿]'))));
      expect(out.apiKeys.any((k) => k.startsWith('ark-9a')), isTrue);
    });

    test('火山 agent plan 全流程：doubao + 双端点 + ark- key + 圈数字剔除', () {
      final out = parsePlatformPaste(
        '火山方舟 Agent Plan 分享\n'
        'Anthropic: https://ark.cn-beijing.volces.com/api/plan\n'
        'OpenAI: https://ark.cn-beijing.volces.com/api/plan/v3\n'
        'apikey: ark-9a②96aed-4c0e-474c-9c09-49⑤8⑨1a00fef-7c3c6（圈数字换成1以此类推）',
        presets,
      );
      expect(out.platform?.value, 'doubao');
      final urls = urlsOf(out);
      expect(urls, contains('https://ark.cn-beijing.volces.com/api/plan'));
      expect(urls, contains('https://ark.cn-beijing.volces.com/api/plan/v3'));
      expect(out.apiKeys.any((k) => k.startsWith('ark-9a96aed')), isTrue);
    });

    test('coding plan 不回归：/api/coding 文案仍命中 doubao，且不误抽 /api/plan', () {
      final out = parsePlatformPaste(
        '火山方舟 CodingPlan\n'
        'Anthropic: https://ark.cn-beijing.volces.com/api/coding\n'
        'OpenAI: https://ark.cn-beijing.volces.com/api/coding/v3\n'
        'key: sk-volces-1234567890abcdef',
        presets,
      );
      expect(out.platform?.value, 'doubao');
      final urls = urlsOf(out);
      expect(urls, contains('https://ark.cn-beijing.volces.com/api/coding'));
      expect(urls, contains('https://ark.cn-beijing.volces.com/api/coding/v3'));
      expect(urls.any((u) => u.contains('/api/plan')), isFalse);
    });

    test('火山双 base_url 各自保留、不塌缩', () {
      final out = parsePlatformPaste(
        '火山方舟 CodingPlan Lite\n'
        'Anthropic: https://ark.cn-beijing.volces.com/api/coding\n'
        'OpenAI: https://ark.cn-beijing.volces.com/api/coding/v3\n'
        'key: sk-volces-1234567890abcdef',
        presets,
      );
      final urls = urlsOf(out);
      expect(urls, contains('https://ark.cn-beijing.volces.com/api/coding'));
      expect(urls, contains('https://ark.cn-beijing.volces.com/api/coding/v3'));
      expect(out.platform?.value, 'doubao');
    });
  });

  group('过期时间识别', () {
    String two(int n) => n.toString().padLeft(2, '0');

    test('「即将过期 MM-DD HH:MM」', () {
      final now = DateTime.now();
      final future = DateTime(now.year, now.month + 1, 28, 23, 59);
      final txt =
          '分享 MIMO token 即将过期 ${two(future.month)}-${two(future.day)} 23:59';
      final out = parsePlatformPaste(txt, presets);
      expect(out.expiresAt, isNotNull);
      expect(out.expiresAt! > DateTime.now().millisecondsSinceEpoch, isTrue);
      final parsed = DateTime.fromMillisecondsSinceEpoch(out.expiresAt!);
      expect(parsed.month, future.month);
      expect(parsed.day, future.day);
      expect(parsed.hour, 23);
      expect(parsed.minute, 59);
    });

    test('超过 7 天的历史日期不回填', () {
      final lastYear = DateTime.now().year - 1;
      final out = parsePlatformPaste('老帖 过期 $lastYear-01-01 00:00', presets);
      expect(out.expiresAt, isNull);
    });

    test('日期级候选（无时间）→ 当日 23:59:59.999', () {
      final now = DateTime.now();
      final future = DateTime(now.year, now.month + 1, 28);
      final out = parsePlatformPaste(
        '即将过期 ${two(future.month)}-${two(future.day)}',
        presets,
      );
      expect(out.expiresAt, isNotNull);
      final parsed = DateTime.fromMillisecondsSinceEpoch(out.expiresAt!);
      expect(parsed.month, future.month);
      expect(parsed.day, future.day);
      expect(parsed.hour, 23);
      expect(parsed.minute, 59);
      expect(parsed.second, 59);
      expect(parsed.millisecond, 999);
    });

    test('YYYY-MM-DD 无时间分量也走当日末刻', () {
      final now = DateTime.now();
      final out = parsePlatformPaste(
        '过期 ${now.year + 1}-${two(now.month)}-${two(now.day)}',
        presets,
      );
      expect(out.expiresAt, isNotNull);
      final parsed = DateTime.fromMillisecondsSinceEpoch(out.expiresAt!);
      expect(parsed.hour, 23);
      expect(parsed.minute, 59);
      expect(parsed.second, 59);
      expect(parsed.millisecond, 999);
    });

    test('带时间分量的候选保留原时间', () {
      final now = DateTime.now();
      final future = DateTime(now.year, now.month + 1, 15, 18, 30);
      final out = parsePlatformPaste(
        '过期 ${two(future.month)}-${two(future.day)} 18:30',
        presets,
      );
      expect(out.expiresAt, isNotNull);
      final parsed = DateTime.fromMillisecondsSinceEpoch(out.expiresAt!);
      expect(parsed.hour, 18);
      expect(parsed.minute, 30);
      expect(parsed.second, 0);
    });

    test('多个候选时取离语义词最近的那个', () {
      final now = DateTime.now();
      final near = DateTime(now.year, now.month + 1, 15, 23, 59);
      final far = DateTime(now.year, now.month + 1, 25, 23, 59);
      final txt =
          '过期 ${two(near.month)}-${two(near.day)} 23:59\n'
          '另一条信息 ${two(far.month)}-${two(far.day)} 23:59';
      final out = parsePlatformPaste(txt, presets);
      expect(out.expiresAt, isNotNull);
      expect(DateTime.fromMillisecondsSinceEpoch(out.expiresAt!).day, 15);
    });

    test('没有过期语义词 → null（收紧模式）', () {
      final now = DateTime.now();
      final future = DateTime(now.year, now.month + 1, 10, 12, 0);
      final out = parsePlatformPaste(
        '活动 ${two(future.month)}-${two(future.day)} 12:00 开始',
        presets,
      );
      expect(out.expiresAt, isNull);
    });

    test('有语义词但日期离它超过 60 字符 → null', () {
      final now = DateTime.now();
      final future = DateTime(now.year, now.month + 1, 15, 23, 59);
      final txt = '过期${'x' * 70}${two(future.month)}-${two(future.day)} 23:59';
      expect(parsePlatformPaste(txt, presets).expiresAt, isNull);
    });

    test('「更新于 YYYY-MM-DD」无语义词 → null', () {
      expect(
        parsePlatformPaste('更新于 2026-07-15 by lin2101', presets).expiresAt,
        isNull,
      );
    });
  });

  group('extractExpiryAt — MM.DD（月.日）格式', () {
    // 固定 now = 2026-06-25 12:00 本地，避免跨日 / 跨年漂移。
    final nowMs = DateTime(2026, 6, 25, 12, 0).millisecondsSinceEpoch;

    test('「PRO套餐，6.27到期」→ 2026-06-27 23:59:59.999', () {
      final ts = extractExpiryAt('分享一个MIMO key，PRO套餐，6.27到期', nowMs);
      expect(ts, isNotNull);
      final d = DateTime.fromMillisecondsSinceEpoch(ts!);
      expect(d.year, 2026);
      expect(d.month, 6);
      expect(d.day, 27);
      expect(d.hour, 23);
      expect(d.minute, 59);
      expect(d.second, 59);
      expect(d.millisecond, 999);
    });

    test('语义词在前（「过期 6.27」）同样识别', () {
      final ts = extractExpiryAt('过期 6.27', nowMs);
      expect(ts, isNotNull);
      final d = DateTime.fromMillisecondsSinceEpoch(ts!);
      expect(d.month, 6);
      expect(d.day, 27);
      expect(d.hour, 23);
    });

    test('无语义词的「6.27」→ null（硬门仍生效）', () {
      expect(extractExpiryAt('版本 Claude 4.5 发布', nowMs), isNull);
      expect(extractExpiryAt('随机文案 6.27 普通文字', nowMs), isNull);
    });

    test('「12.31到期」当年未过 → 当年 12-31', () {
      final ts = extractExpiryAt('12.31到期', nowMs);
      expect(ts, isNotNull);
      final d = DateTime.fromMillisecondsSinceEpoch(ts!);
      expect(d.year, 2026);
      expect(d.month, 12);
      expect(d.day, 31);
      expect(d.hour, 23);
      expect(d.minute, 59);
    });

    test('「1.15到期」当年已过 → 推次年 2027-01-15', () {
      final ts = extractExpiryAt('1.15到期', nowMs);
      expect(ts, isNotNull);
      final d = DateTime.fromMillisecondsSinceEpoch(ts!);
      expect(d.year, 2027);
      expect(d.month, 1);
      expect(d.day, 15);
      expect(d.hour, 23);
    });
  });

  group('collectKeyPrefixes — 数据驱动，禁代码硬编码平台前缀', () {
    test('从 presets 的 key_prefixes 收集，含通用 sk-，长在前', () {
      final p = collectKeyPrefixes(presets);
      expect(p, contains('sk-ant-'));
      expect(p, contains('ark-'));
      expect(p, contains('tp-'));
      expect(p, contains('sk-'));
      expect(p.indexOf('sk-ant-'), lessThan(p.indexOf('sk-')));
    });

    test('没有平台前缀时至少返回通用两条', () {
      expect(
        collectKeyPrefixes(const [PastePresetRef(value: 'x', label: 'x')]),
        ['sk-', 'sk_'],
      );
    });
  });
}
