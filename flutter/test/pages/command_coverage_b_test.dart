/// 页面批次 B（平台 / 分组 / 请求日志 / 日志）的命令覆盖零差集自证 —— 本票的核心验收项。
///
/// 做法与票 I08 同一套：React 侧这四页**实际调用**的命令名由
/// `scripts/i07-react-pages-b-commands.mjs` 从 `src/` 解析出来（命名空间对象只算
/// 本文件真访问过的成员，所以 import 一个 `platformApi` 不会把它 20 个方法全算进来），
/// 清单落在 `test/pages/react_pages_b_commands.txt`；这里与 Flutter 侧四个逻辑文件里
/// 出现的命令名字面量做零差集比对。
///
/// 差一条就是功能缺一块 —— 这条测试红了**不许改清单**，要去把那个命令补上。
/// 清单本身由脚本生成，`--check` 能在 React 加了新调用而清单没更新时报出来。
///
/// 比的是**排序后的 List**，不是 Set：Dart 的 Set / List / Map 按标识比较，
/// `expect(setA, setB)` 在内容相同时也会判不等（本项目踩过，2826 个 key 全报不等）。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 从 Dart 源码里抠出所有 snake_case 字符串字面量，与 I01 的
/// `test/command_names_test.dart`、I08 的 `test/settings/command_coverage_test.dart`
/// 同一套路。先剥掉注释，免得注释里提到的命令名被当成实现。
Set<String> commandLiteralsIn(String source) {
  final code = source
      .split('\n')
      .map((l) {
        final i = l.indexOf('//');
        return i < 0 ? l : l.substring(0, i);
      })
      .join('\n');
  final out = <String>{};
  for (final m in RegExp(r"'([a-z][a-z0-9]*(?:_[a-z0-9]+)+)'").allMatches(code)) {
    out.add(m.group(1)!);
  }
  return out;
}

/// 本票交付的 Flutter 侧文件。写死清单而不是扫整个 `lib/src/pages/` ——
/// 那样会把 I06 的首页 / 统计页也算进来，它们调的 `proxy_status` 之类不在本批次里，
/// 「多出来的命令」那一栏就会变成一堆噪音，真有问题时反而看不见。
const List<String> kBatchBFiles = [
  'lib/src/pages/platforms_logic.dart',
  'lib/src/pages/groups_logic.dart',
  'lib/src/pages/logs_logic.dart',
];

void main() {
  // 测试的 cwd 是 flutter/。
  final listFile = File('test/pages/react_pages_b_commands.txt');

  test('清单文件在（由脚本生成，禁手改）', () {
    expect(
      listFile.existsSync(),
      isTrue,
      reason: '跑 `node scripts/i07-react-pages-b-commands.mjs` 生成',
    );
    for (final p in kBatchBFiles) {
      expect(File(p).existsSync(), isTrue, reason: '缺文件：$p');
    }
  });

  test('React 这四页调用的每个命令，Flutter 侧都有调用点（零差集）', () {
    final expected = listFile
        .readAsLinesSync()
        .map((l) => l.trim())
        .where((l) => l.isNotEmpty && !l.startsWith('#'))
        .toSet();

    final actual = <String>{};
    for (final p in kBatchBFiles) {
      actual.addAll(commandLiteralsIn(File(p).readAsStringSync()));
    }

    final missing = expected.difference(actual).toList()..sort();

    // 已知且故意不覆盖的，逐条写明理由。**写在这里 = 被数进去了**，
    // 不是从清单里悄悄删掉；上面 missing 的长度仍然按全量算。
    const knownGaps = <String, String>{
      'get_client_types_json':
          '端点的「客户端形态」下拉。registry 已删 client_type 字段，'
          '形态按 protocol 派生（CLAUDE.md 的 derive_client_type），本层不读这份表。',
      'settings_get':
          '被 domains/groups/proxy-env.ts 调，用来拼分组的环境变量预览文本。'
          '本层不做那个预览面板。',
      'sync_group_settings':
          '把分组配置写进 ~/.claude/settings.{group}.json。属「一键同步」按钮，'
          '不是分组 CRUD 的一部分；未随本票交付。',
    };

    final realMissing = [
      for (final c in missing)
        if (!knownGaps.containsKey(c)) c,
    ];
    final acknowledged = [
      for (final c in missing)
        if (knownGaps.containsKey(c)) c,
    ];

    expect(
      realMissing,
      isEmpty,
      reason:
          'React 调了、Flutter 没调、也没写明理由 —— 每一条都是用户少掉的一个功能：\n'
          '${realMissing.join('\n')}',
    );

    // 让缺口被数出来、印出来，而不是消失在清单里。
    // ignore: avoid_print
    print(
      '[I07 命令覆盖] React 清单 ${expected.length} 条；'
      'Flutter 覆盖 ${expected.length - acknowledged.length} 条；'
      '已声明未覆盖 ${acknowledged.length} 条：${acknowledged.join(', ')}',
    );
    expect(acknowledged.length, knownGaps.length,
        reason: '已声明的缺口数与 knownGaps 表对不上 —— 说明表里有条目已经被实现了，去掉它');
  });

  test('命令名字面量提取器本身是对的', () {
    expect(
      commandLiteralsIn("await _invoke('platform_list');"),
      contains('platform_list'),
    );
    expect(
      commandLiteralsIn("_invoke(x ? 'group_create' : 'group_update')"),
      containsAll(<String>['group_create', 'group_update']),
    );
    // 注释里提到的不算实现。
    expect(commandLiteralsIn("// 以后要调 'not_a_cmd'"), isEmpty);
    // 单段小写词不是命令名形状，不收。
    expect(commandLiteralsIn("const x = 'hello';"), isEmpty);
  });
}
