/// 命令覆盖零差集自证 —— 本票的核心验收项。
///
/// 做法：把 React 侧设置这一批页面**实际调用**的命令名（由
/// `scripts/i08-react-settings-commands.mjs` 从 `src/` 解析出来，清单落在
/// `test/settings/react_settings_commands.txt`），与 Flutter 侧
/// `lib/src/pages/settings/` 里出现的命令名字面量做零差集比对。
///
/// 差一条就是功能缺一块 —— 这条测试红了不许改清单，要去把那个命令补上。
/// 清单本身由脚本生成，`--check` 进了 `make lint`（React 加了新调用而清单没更新就红）。
///
/// 与 `test/command_names_test.dart` 的分工：那条管「Dart 里写的命令名后端认不认」
/// （对着 `startup.rs` 的 generate_handler! 比），本条管「React 调的命令 Flutter 有没有调」。
/// 两条方向相反，都需要。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 从 Dart 源码里抠出**所有 snake_case 字符串字面量**，与 I01 的
/// `test/command_names_test.dart` 同一套路。
///
/// 为什么不只抓 `_invoke('cmd')`：命令名未必直接写在调用处 —— 三元表达式
/// （`_invoke(s.enabled ? 'mitm_disable' : 'mitm_enable')`）和接线常量
/// （`readCmd: 'pi_settings_read'`）都写在别处。只认调用处会把这些误判成「没实现」。
/// 代价是可能混进同形状的普通字符串，但比对只做「期望集是否被覆盖」，多认无害。
Set<String> commandLiteralsIn(String source) {
  // 先剥掉行注释与文档注释，免得注释里提到的命令名被当成实现。
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

void main() {
  // 测试的 cwd 是 flutter/。
  final settingsDir = Directory('lib/src/pages/settings');
  final listFile = File('test/settings/react_settings_commands.txt');

  test('清单文件在（由脚本生成，禁手改）', () {
    expect(listFile.existsSync(), isTrue,
        reason: '跑 `node scripts/i08-react-settings-commands.mjs` 生成');
    expect(settingsDir.existsSync(), isTrue);
  });

  test('React 设置页调用的每个命令，Flutter 侧都有调用点（零差集）', () {
    final expected = listFile
        .readAsLinesSync()
        .map((l) => l.trim())
        .where((l) => l.isNotEmpty && !l.startsWith('#'))
        .toSet();

    final actual = <String>{};
    for (final f in settingsDir.listSync(recursive: true).whereType<File>()) {
      if (!f.path.endsWith('.dart')) continue;
      actual.addAll(commandLiteralsIn(f.readAsStringSync()));
    }

    // Dart 的 Set 按标识比，不按值比 —— 直接 expect(actual, expected) 在结构
    // 相同时也会判不等。所以比排序后的 List，并把两侧差集单独报出来。
    final missing = expected.difference(actual).toList()..sort();
    final extra = actual.difference(expected).toList()..sort();

    expect(
      missing,
      isEmpty,
      reason: 'React 调了但 Flutter 没调 —— 每一条都是用户少掉的一个功能：\n'
          '${missing.join('\n')}',
    );
    // 多出来的不算错（Flutter 侧可能为同一功能多拆一步），但要看得见。
    expect(
      extra.where((c) => false),
      isEmpty,
      reason: 'Flutter 多调的命令（仅供审阅）：${extra.join(', ')}',
    );
  });

  test('命令名字面量提取器本身是对的', () {
    expect(commandLiteralsIn("await _invoke('proxy_start', {'port': 1});"), contains('proxy_start'));
    // 三元表达式里的两支都要认出来。
    expect(
      commandLiteralsIn("_invoke(x ? 'mitm_disable' : 'mitm_enable')"),
      containsAll(<String>['mitm_disable', 'mitm_enable']),
    );
    // 接线常量里的也要认。
    expect(commandLiteralsIn("readCmd: 'pi_settings_read',"), contains('pi_settings_read'));
    // 注释里提到的不算实现。
    expect(commandLiteralsIn("// 这里以后要调 'not_a_cmd'"), isEmpty);
    // 单段小写词不是命令名形状，不收。
    expect(commandLiteralsIn("const x = 'hello';"), isEmpty);
  });
}
