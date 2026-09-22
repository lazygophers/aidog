/// JSON 解析错误的行列定位。
///
/// 语法高亮 / 行号 / 折叠都由 `re_editor` 提供（票 27 第 2 步换的包），**只有报错
/// 定位它不管** —— 它高亮语法，不校验 JSON 合法性。所以这一件留在自己手里：
/// `jsonDecode` 抛的 `FormatException` 带字符偏移，换算成行列摆给用户。
///
/// 早先自写的分词器与高亮 controller 随换包一并删除，不留两套。
library;

import 'dart:convert';

// ── 错误定位 ──────────────────────────────────────────────────────

/// 一次 JSON 解析失败的位置。[line] / [column] 从 1 起（编辑器惯例）。
class JsonErrorLocation {
  const JsonErrorLocation({
    required this.line,
    required this.column,
    required this.message,
  });

  final int line;
  final int column;

  /// 原始报错消息（已剔掉 Dart 自带的 `at character N` 尾巴 —— 字符偏移对人没用，
  /// 行列才有用，而且两个位置摆在一起会互相打架）。
  final String message;

  /// 摆到界面上的一行。`L12:5` 是编辑器通用写法，不进 i18n。
  String get label => 'L$line:$column $message';

  @override
  String toString() => label;
}

/// 字符偏移 → 行列（都从 1 起）。
({int line, int column}) lineColumnAt(String source, int offset) {
  final o = offset.clamp(0, source.length);
  var line = 1;
  var lineStart = 0;
  for (var i = 0; i < o; i++) {
    if (source.codeUnitAt(i) == 0x0a) {
      line++;
      lineStart = i + 1;
    }
  }
  return (line: line, column: o - lineStart + 1);
}

/// 解析这段 JSON；能解就返回 null，解不了就返回出错的行列与原因。
///
/// 空串当**合法**（= 这个字段没配），与 `_JsonField` 的写回约定一致。
JsonErrorLocation? locateJsonError(String source) {
  if (source.trim().isEmpty) return null;
  try {
    jsonDecode(source);
    return null;
  } on FormatException catch (e) {
    final at = lineColumnAt(source, e.offset ?? 0);
    return JsonErrorLocation(
      line: at.line,
      column: at.column,
      message: _cleanMessage(e.message),
    );
  }
}

/// Dart 的 `FormatException.message` 干净，但 `toString()` 会缀上
/// `at character N`。这里只取 message，并去掉可能出现的尾部位置信息。
String _cleanMessage(String raw) =>
    raw.replaceFirst(RegExp(r'\s*at character \d+\.?$'), '').trim();
