// 8 语言文案层。证的两件事：
// 1. 8 种语言都能加载、键集完全一致 —— 任何一种语言少一个键，切过去就是裸 key。
// 2. 文案真值源只有一份：断言资产里的 JSON 与 src-tauri/crates/aidog_i18n/locales/
//    是同一批文件（内容逐字节相同），仓库里没有会漂移的第二份拷贝。

import 'dart:io';

import 'package:aidog_flutter/i18n.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

/// 从 flutter/ 往上找仓库根（有 src-tauri/ 的那一层）。
Directory repoRoot() {
  var dir = Directory.current.absolute;
  while (true) {
    if (Directory('${dir.path}/src-tauri').existsSync()) return dir;
    final parent = dir.parent;
    if (parent.path == dir.path) {
      throw StateError(
        'repo root (the directory holding src-tauri/) not found',
      );
    }
    dir = parent;
  }
}

String sourcePath(String locale) =>
    '${repoRoot().path}/src-tauri/crates/aidog_i18n/locales/$locale.json';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(Translations.clearCache);

  group('文案来源', () {
    test('8 种语言的资产逐字节等于 Rust crate 里的真值源', () async {
      for (final locale in kAllLocales) {
        final asset = await rootBundle.loadString(
          '$kLocaleAssetDir$locale.json',
        );
        final source = File(sourcePath(locale)).readAsStringSync();
        expect(asset, source, reason: '$locale 的资产与真值源不一致 —— 说明某处多出了一份拷贝');
      }
    });

    test('kAllLocales 覆盖 locales/ 目录里的全部 JSON，不多不少', () {
      final onDisk =
          Directory('${repoRoot().path}/src-tauri/crates/aidog_i18n/locales')
              .listSync()
              .whereType<File>()
              .where((f) => f.path.endsWith('.json'))
              .map((f) => f.uri.pathSegments.last.replaceAll('.json', ''))
              .toSet();
      expect(onDisk, kAllLocales.toSet());
    });
  });

  group('键集对齐', () {
    late Map<String, Translations> all;

    setUp(() async {
      all = <String, Translations>{
        for (final l in kAllLocales) l: await Translations.load(l),
      };
    });

    test('8 种语言的键集完全一致', () {
      final base = all[kFallbackLocale]!.entries.keys.toSet();
      // 2826（I03 落地时）+ 8（票 I07 给分组 / 平台 / 日志三页补的新词条）
      // + 4（票 I15 托盘三段：keptDisabled / pickAtMost / segment.peak / segment.routed）
      // + 1（nav.collapse —— 侧栏「收起」按钮的文案，8 个 locale 里原先全缺，界面上直接显裸 key）
      // + 2（platform.deleteTitle / platform.deleteConfirm —— 删平台的二次确认，
      //      2026-09-22 两侧同用；此前 Flutter 借用 group.deletePlatformConfirm，
      //      那条文案写着「仅属此分组」，在平台列表里根本不成立）。
      // + 1（importExport.renameRequired —— key 仍在 8 个 locale 里；
      //      2026-09-24 对齐 React 后 UI 不再用它，删 key 要 8 份一起动，暂留）。
      // + 1（platform.numberInvalid —— 数字框打错时的提示。React 那边靠
      //      `<input type="number">` 由浏览器拦，Flutter 没有等价物，必须自己说）。
      // + 9（platform.quotaSection.* —— 配额查询合区 Tab + 确认弹窗，quota-ia 14）。
      // 这个数是故意写死的：加 key 必须 8 个 locale 一起加，改这一行时就会想起来。
      //
      // 数的是 [Translations.flatten] **之后**的键数，不是 JSON 顶层键数 ——
      // `group` / `logs` 在 JSON 里是嵌套对象，拍平后会展开成多条。拿
      // `len(json.load(f))` 去对这个数一定对不上，别那样核。
      expect(base, hasLength(2855));
      for (final locale in kAllLocales) {
        final keys = all[locale]!.entries.keys.toSet();
        expect(keys.difference(base), isEmpty, reason: '$locale 多出 en-US 没有的键');
        expect(base.difference(keys), isEmpty, reason: '$locale 缺键，切过去会显裸 key');
      }
    });

    test('没有空文案（空串等于裸 key 的另一种形态）', () {
      for (final locale in kAllLocales) {
        final blank = all[locale]!.entries.entries
            .where((e) => e.value.trim().isEmpty)
            .map((e) => e.key)
            .toList();
        expect(blank, isEmpty, reason: '$locale 有空文案: $blank');
      }
    });

    test('每种语言都真的译过（不是整份照抄 en-US）', () {
      final en = all['en-US']!.entries;
      for (final locale in kAllLocales.where((l) => l != 'en-US')) {
        final same = all[locale]!.entries.entries
            .where((e) => e.value == en[e.key])
            .length;
        // 产品名、协议名、单位这类本来就不该翻，允许相当一部分相同；
        // 整份一模一样才说明这个语言根本没接上。
        expect(same, lessThan(en.length), reason: '$locale 与 en-US 一字不差');
      }
    });
  });

  group('占位符', () {
    test('{{占位符}} 在 8 种语言里成套出现', () async {
      final re = RegExp(r'\{\{\s*([^{}]+?)\s*\}\}');
      // 归成一个**字符串**再比：Dart 的 Set / List / Map 都是身份相等，
      // 两个内容相同的 Set 彼此 != ，拿它当判据会让每个 key 都被判成「不匹配」。
      // 只有 String 是值相等。
      String varsOf(String s) =>
          (re.allMatches(s).map((m) => m.group(1)!).toSet().toList()..sort())
              .join('|');
      final en = (await Translations.load('en-US')).entries;
      final mismatched = <String>[];
      for (final locale in kAllLocales.where((l) => l != 'en-US')) {
        (await Translations.load(locale)).entries.forEach((key, value) {
          if (varsOf(value) != varsOf(en[key]!)) mismatched.add('$locale/$key');
        });
      }
      expect(mismatched, isEmpty, reason: '译文占位符与 en-US 对不上，运行时会漏值');
    });
  });

  group('拍平与查表', () {
    test('嵌套写法优先于同名平铺键（照 i18next 的解析顺序）', () {
      // en-US.json 里 group.addEnvVar 两种写法并存且文案不同，
      // 嵌套是 'Add env var'，平铺是 'Add environment variable'。
      final flat = Translations.flatten(<String, dynamic>{
        'a.b': 'flat',
        'a': <String, dynamic>{'b': 'nested', 'c': 'only-nested'},
      });
      expect(flat['a.b'], 'nested');
      expect(flat['a.c'], 'only-nested');
    });

    test('真文件里那 7 个双写键取的是嵌套值', () async {
      final en = await Translations.load('en-US');
      expect(en.entries['group.addEnvVar'], 'Add env var');
    });

    test('查不到 → 回落 en-US → 再查不到返回键本身', () {
      const cur = Translations('ar-SA', <String, String>{'a': 'ع'});
      const fb = Translations('en-US', <String, String>{'a': 'A', 'b': 'B'});
      expect(translate('a', cur, fb), 'ع');
      expect(translate('b', cur, fb), 'B');
      expect(translate('nope', cur, fb), 'nope');
    });

    test('插值只认双花括号，缺参数原样留着', () {
      expect(interpolate('v{{version}}', {'version': '1.2'}), 'v1.2');
      expect(interpolate('v{{ version }}', {'version': '1.2'}), 'v1.2');
      expect(interpolate('{{a}}-{{b}}', {'a': 1, 'b': 2}), '1-2');
      expect(interpolate('{{a}}', {}), '{{a}}');
      // 单花括号是 JSON 里的历史写法，i18next 不认，这里同样不认。
      expect(interpolate('{count}', {'count': 3}), '{count}');
    });
  });

  group('locale 归一化', () {
    test('历史值与脏值都落到 8 种之一', () {
      expect(normalizeLocale('zh-CN'), 'zh-Hans');
      expect(normalizeLocale('zh'), 'zh-Hans');
      expect(normalizeLocale('ar'), 'ar-SA');
      expect(normalizeLocale('ES_es'), 'es-ES');
      expect(normalizeLocale('klingon'), kDefaultLocale);
      expect(normalizeLocale(null), kDefaultLocale);
      expect(normalizeLocale(42), kDefaultLocale);
      for (final l in kAllLocales) {
        expect(normalizeLocale(l), l);
      }
    });

    test('系统语言映射', () {
      expect(localeFromPlatform(const Locale('zh', 'TW')), 'zh-Hans');
      expect(localeFromPlatform(const Locale('ar', 'EG')), 'ar-SA');
      expect(localeFromPlatform(const Locale('en', 'GB')), 'en-US');
      expect(localeFromPlatform(const Locale('sv', 'SE')), kDefaultLocale);
    });

    test('只有 ar-SA 是 RTL', () {
      for (final l in kAllLocales) {
        expect(isRtlLocale(l), l == 'ar-SA', reason: l);
        expect(
          textDirectionOf(l),
          l == 'ar-SA' ? TextDirection.rtl : TextDirection.ltr,
        );
      }
    });

    test('flutterLocaleOf 给 MaterialApp 用的形状', () {
      expect(flutterLocaleOf('zh-Hans').toLanguageTag(), 'zh-Hans');
      expect(flutterLocaleOf('ar-SA').toLanguageTag(), 'ar-SA');
      expect(kSupportedFlutterLocales, hasLength(8));
    });
  });

  group('切语言', () {
    late I18nController c;
    final persisted = <String>[];

    setUp(() {
      persisted.clear();
      c = I18nController(
        persist: (l) async {
          persisted.add(l);
        },
      );
    });

    test('init 之前调 t() 当场抛，而不是悄悄出裸 key', () {
      expect(() => c.t('common.loading'), throwsStateError);
    });

    test('8 种语言逐个切过去，取到的都是本语言的文案', () async {
      await c.init(initial: 'en-US');
      expect(c.t('common.loading'), 'Loading...');
      final seen = <String, String>{};
      for (final locale in kAllLocales) {
        await c.setLocale(locale);
        expect(c.locale, locale);
        // lang.<code> 是语言自己的名字，8 种语言里都有，最适合当探针。
        seen[locale] = c.t('lang.$locale');
      }
      expect(seen['zh-Hans'], '简体中文');
      expect(seen['ar-SA'], 'العربية');
      expect(seen['ja-JP'], '日本語');
      expect(seen['ru-RU'], 'Русский');
      expect(persisted, kAllLocales);
    });

    test('切到脏值归一化，切到同一语言不重复通知', () async {
      await c.init(initial: 'en-US');
      var notifications = 0;
      c.addListener(() => notifications++);
      await c.setLocale('zh-CN');
      expect(c.locale, 'zh-Hans');
      expect(notifications, 1);
      await c.setLocale('zh-Hans');
      expect(notifications, 1);
    });

    test('写 DB 失败不把界面按回去', () async {
      final broken = I18nController(
        persist: (_) => throw StateError('db down'),
      );
      await broken.init(initial: 'en-US');
      await broken.setLocale('de-DE');
      expect(broken.locale, 'de-DE');
    });

    test('插值走到 t()', () async {
      await c.init(initial: 'en-US');
      expect(
        c.t('about.downloading', {'version': '0.1.17'}),
        'Downloading v0.1.17',
      );
    });
  });

  group('8 种语言都能渲染，且没有裸 key', () {
    const probes = <String>[
      'common.loading',
      'common.cancel',
      'lang.label',
      'kernel.authToken',
      'about.appVersion',
      'stats.loading',
      'platform.usageLoading',
      'modality.embedding',
      'group.addEnvVar',
      'logs.cacheTokens',
    ];

    for (final locale in kAllLocales) {
      testWidgets(locale, (tester) async {
        final c = I18nController(persist: (_) async {});
        // 真实资产加载要走 runAsync：直接 await 会和 pumpWidget 的守卫撞
        // （flutter_test 的 "Guarded function conflict"）。
        await tester.runAsync(() => c.init(initial: locale));
        final keys = <String>[...probes, 'lang.$locale'];
        await tester.pumpWidget(
          AidogI18n(
            controller: c,
            child: Column(
              children: [
                for (final k in keys) Text(c.t(k), key: ValueKey<String>(k)),
              ],
            ),
          ),
        );

        const unfilled = '{{';
        for (final k in keys) {
          final w = tester.widget<Text>(find.byKey(ValueKey<String>(k)));
          final rendered = w.data!;
          final where = '$locale 的 $k';
          expect(rendered.trim(), isNotEmpty, reason: '$where 渲染成空');
          expect(rendered, isNot(k), reason: '$where 显的是裸 key');
          expect(
            rendered,
            isNot(contains(unfilled)),
            reason: '$where 留着没填的占位符',
          );
        }

        const probeKey = ValueKey<String>('lang.label');
        final el = tester.element(find.byKey(probeKey));
        expect(
          Directionality.of(el),
          locale == 'ar-SA' ? TextDirection.rtl : TextDirection.ltr,
        );
      });
    }

    testWidgets('切语言后整棵树跟着换，方向也跟着翻', (tester) async {
      final c = I18nController(persist: (_) async {});
      // 真实资产加载走 runAsync，理由同上（Guarded function conflict）。
      await tester.runAsync(() => c.init(initial: 'en-US'));
      const probeKey = ValueKey<String>('probe');
      await tester.pumpWidget(
        AidogI18n(
          controller: c,
          // 经由 context 取词 —— 这正是切语言能自动重建的唯一路径。
          child: Builder(
            builder: (context) =>
                Text(AidogI18n.of(context).t('common.cancel'), key: probeKey),
          ),
        ),
      );
      expect(find.text('Cancel'), findsOneWidget);
      expect(
        Directionality.of(tester.element(find.byKey(probeKey))),
        TextDirection.ltr,
      );

      await tester.runAsync(() => c.setLocale('ar-SA'));
      await tester.pump();
      expect(find.text('Cancel'), findsNothing);
      expect(find.text('إلغاء'), findsOneWidget);
      expect(
        Directionality.of(tester.element(find.byKey(probeKey))),
        TextDirection.rtl,
      );
    });
  });
}
