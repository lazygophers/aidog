// 命令名漂移护栏。
//
// I01 的决定是**不生成 203 个 Dart 绑定**，只暴露一个泛型 `kernel.invoke<T>(cmd, args)`
// （理由见 ../README.md）。放弃绑定就放弃了「命令名打错编译期报错」这件事，所以用这条测试补回来：
// Dart 源码里出现的每一个命令名，都必须在 `src-tauri/src/startup.rs` 的 `generate_handler!`
// 里登记过 —— 那是 invoke 名的唯一真值源（见项目 CLAUDE.md）。
//
// 同 idiom 的既有护栏：`scripts/t06-handler-names.mjs` / `scripts/t08-rpc-names.mjs`。

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 从 flutter/ 往上找仓库根（有 src-tauri/ 的那一层）。
Directory repoRoot() {
  var dir = Directory.current.absolute;
  while (true) {
    if (Directory('${dir.path}/src-tauri').existsSync()) return dir;
    final parent = dir.parent;
    if (parent.path == dir.path) {
      throw StateError('repo root (the directory holding src-tauri/) not found');
    }
    dir = parent;
  }
}

/// `generate_handler![ ... ]` 里登记的命令名。invoke 名取 `#[tauri::command]` 函数名，
/// 与模块路径无关，所以取每条路径的末段。
Set<String> registeredCommands(Directory root) {
  final src = File('${root.path}/src-tauri/src/startup.rs').readAsStringSync();
  final block = RegExp(
    r'generate_handler!\s*\[([\s\S]*?)\n\s*\]',
  ).firstMatch(src);
  expect(block, isNotNull, reason: 'startup.rs 里找不到 generate_handler![ ... ]');
  final names = <String>{};
  for (final raw in block!.group(1)!.split('\n')) {
    final line = raw.trim();
    if (line.isEmpty || line.startsWith('//')) continue;
    final m = RegExp(r'^([A-Za-z0-9_:]+),$').firstMatch(line);
    if (m == null) continue;
    names.add(m.group(1)!.split('::').last);
  }
  return names;
}

/// Dart 源码里所有 `invoke<...>('cmd'` / `invoke('cmd'` 的字面量命令名。
/// 变量传进去的取不到 —— 那种写法本来也就绕过了这条护栏，别写。
Map<String, List<String>> invokedCommands(Directory libDir) {
  // `<...>` 里可能嵌套（`invoke<Map<String, dynamic>>`），所以按「不含括号的任意串」匹配，
  // 不能用 `[^>]*`。
  final pattern = RegExp("""invoke(?:<[^()]*>)?\\(\\s*['"]([a-z0-9_]+)['"]""");
  final found = <String, List<String>>{};
  for (final f in libDir.listSync(recursive: true).whereType<File>()) {
    if (!f.path.endsWith('.dart')) continue;
    for (final m in pattern.allMatches(f.readAsStringSync())) {
      found.putIfAbsent(m.group(1)!, () => <String>[]).add(f.path);
    }
  }
  return found;
}

void main() {
  test('generate_handler! 里有 203 个命令（spec §1.2 的实测值）', () {
    expect(registeredCommands(repoRoot()).length, 203);
  });

  test('Dart 侧调的每个命令名都在 generate_handler! 里登记过', () {
    final root = repoRoot();
    final registered = registeredCommands(root);
    final used = invokedCommands(Directory('${root.path}/flutter/lib'));
    expect(used, isNotEmpty, reason: 'lib/ 里一个命令都没调到，正则八成失效了');
    final unknown = <String>[];
    used.forEach((cmd, files) {
      if (!registered.contains(cmd)) unknown.add('$cmd (${files.join(", ")})');
    });
    expect(
      unknown,
      isEmpty,
      reason: '这些命令名后端没有，运行时会 404：\n${unknown.join("\n")}',
    );
  });
}
