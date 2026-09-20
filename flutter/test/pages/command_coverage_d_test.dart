/// 页面批次 D（技能 / MCP / 关于 / 通知 / 模型信息）的命令覆盖零差集自证 —— 本票的核心验收项。
///
/// 做法与票 I07 / I08 同一套：React 侧这批页面**实际调用**的命令名由
/// `scripts/i09-react-pages-d-commands.mjs` 从 `src/` 解析出来（共用
/// `scripts/lib/react-commands.mjs`；命名空间对象只算本文件真访问过的成员，
/// 所以 import 一个 `skillsApi` 不会把它 17 个方法全算进来），清单落在
/// `test/pages/react_pages_d_commands.txt`；这里与 Flutter 侧本票交付的文件里
/// 出现的命令名字面量做零差集比对。
///
/// 差一条就是功能缺一块 —— 这条测试红了**不许改清单**，要去把那个命令补上。
///
/// 比的是**排序后的 List**，不是 Set：Dart 的 Set / List / Map 按标识比较，
/// `expect(setA, setB)` 在内容相同时也会判不等（本项目踩过，2826 个 key 全报不等）。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 从 Dart 源码里抠出所有 snake_case 字符串字面量，与票 I07 的
/// `command_coverage_b_test.dart` 同一套路。先剥掉注释，免得注释里提到的命令名
/// 被当成实现。
Set<String> commandLiteralsIn(String source) {
  final code = source
      .split('\n')
      .map((l) {
        final i = l.indexOf('//');
        return i < 0 ? l : l.substring(0, i);
      })
      .join('\n');
  final out = <String>{};
  for (final m in RegExp(
    r"'([a-z][a-z0-9]*(?:_[a-z0-9]+)+)'",
  ).allMatches(code)) {
    out.add(m.group(1)!);
  }
  return out;
}

/// 本票交付的 Flutter 侧文件。写死清单而不是扫整个 `lib/src/pages/` ——
/// 那样会把 I06 / I07 / I08 的页面也算进来，「多出来的命令」那一栏就变成噪音。
const List<String> kBatchDFiles = [
  'lib/src/pages/skills_logic.dart',
  'lib/src/pages/mcp_logic.dart',
  'lib/src/pages/about_logic.dart',
  'lib/src/pages/model_info_logic.dart',
  'lib/src/pages/model_test_logic.dart',
  // 通知中心只有两条命令、无独立逻辑层，命令就在 widget 文件里发。
  'lib/src/pages/notifications.dart',
];

void main() {
  // 测试的 cwd 是 flutter/。
  final listFile = File('test/pages/react_pages_d_commands.txt');

  test('清单文件在（由脚本生成，禁手改）', () {
    expect(
      listFile.existsSync(),
      isTrue,
      reason: '跑 `node scripts/i09-react-pages-d-commands.mjs` 生成',
    );
    for (final p in kBatchDFiles) {
      expect(File(p).existsSync(), isTrue, reason: '缺文件：$p');
    }
  });

  test('React 这批页面调用的每个命令，Flutter 侧都有调用点（零差集）', () {
    final expected = listFile
        .readAsLinesSync()
        .map((l) => l.trim())
        .where((l) => l.isNotEmpty && !l.startsWith('#'))
        .toSet();

    final actual = <String>{};
    for (final p in kBatchDFiles) {
      actual.addAll(commandLiteralsIn(File(p).readAsStringSync()));
    }

    final missing = expected.difference(actual).toList()..sort();

    // 已知且故意不覆盖的，逐条写明理由。**写在这里 = 被数进去了**，
    // 不是从清单里悄悄删掉；上面 missing 的长度仍然按全量算。
    //
    // **本票这张表是空的**：39 条全覆盖，一条都没落下（票 I07 那批有 6 条）。
    // 表留着不是形式主义 —— 下面那条「僵尸条目」断言靠它：往表里写一条其实已经
    // 实现了的命令，`acknowledged` 会比表短，当场红。这一条在开发中真的抓到过一次
    // （占位写了 `skills_install_batch`，而它已经实现）。
    const knownGaps = <String, String>{};

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
      '[I09 命令覆盖] React 清单 ${expected.length} 条；'
      'Flutter 覆盖 ${expected.length - acknowledged.length} 条；'
      '已声明未覆盖 ${acknowledged.length} 条：${acknowledged.join(', ')}',
    );

    // 票 I07 的那条僵尸条目闸：knownGaps 里如果有条目其实已经实现了，
    // acknowledged 就会比表短 —— 当场红，逼着去删表里那一行。
    expect(
      acknowledged.length,
      knownGaps.length,
      reason:
          'knownGaps 表里有条目已经被实现了（或写错了命令名），去掉它：\n'
          '表里有 ${knownGaps.keys.toList()..sort()}\n'
          '实际缺口 $acknowledged',
    );
  });

  test('命令名字面量提取器本身是对的', () {
    expect(
      commandLiteralsIn("await _invoke('skills_list_refresh');"),
      contains('skills_list_refresh'),
    );
    expect(
      commandLiteralsIn("invoke(x ? 'mcp_add' : 'mcp_update')"),
      containsAll(<String>['mcp_add', 'mcp_update']),
    );
    // 注释里提到的不算实现。
    expect(commandLiteralsIn("// 以后要调 'not_a_cmd'"), isEmpty);
    // 单段小写词不是命令名形状，不收。
    expect(commandLiteralsIn("const x = 'hello';"), isEmpty);
  });
}
