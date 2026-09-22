/// JSON 文本的两件纯逻辑（票 27 第 2 步）：**定位解析错误到行列**、**分词上色**。
///
/// React 那边这两件事都由 CodeMirror 代劳（`JsonCodeEditor.tsx` 引了
/// `@codemirror/lang-json` + `@codemirror/lint` + `@lezer/highlight`）。Flutter 的
/// `TextField` 什么都不带，所以自己写：分词器 + 一个重写 `buildTextSpan` 的
/// controller，就能在**不换输入控件、不引新依赖**的前提下拿到高亮与行内报错。
///
/// 折叠（fold）不在这里 —— 那要能把一段文本换成占位并重映射光标偏移，
/// `TextField` 做不到，得换成自绘编辑器或引第三方包。
library;

import 'dart:convert';

import 'package:flutter/material.dart';

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

// ── 分词 ──────────────────────────────────────────────────────────

enum JsonTokenKind { key, string, number, literal, punct, plain }

/// 一段文本的分词结果：`[start, end)` 左闭右开。
class JsonToken {
  const JsonToken(this.start, this.end, this.kind);

  final int start;
  final int end;
  final JsonTokenKind kind;

  @override
  String toString() => '$kind[$start,$end)';
}

const int _quote = 0x22; // "
const int _backslash = 0x5c; // \
const int _colon = 0x3a; // :

/// 把 JSON 文本切成 token。**不要求文本合法** —— 用户正在打字，一半时间它就是
/// 半成品；切不动的部分归 [JsonTokenKind.plain]，照常显示不报错。
///
/// 字符串后面跟的第一个非空白字符是 `:` 就算 key（对象的键），否则算普通字符串。
List<JsonToken> tokenizeJson(String src) {
  final out = <JsonToken>[];
  var i = 0;
  while (i < src.length) {
    final c = src.codeUnitAt(i);
    if (c == _quote) {
      final start = i;
      i++;
      while (i < src.length) {
        final ch = src.codeUnitAt(i);
        if (ch == _backslash) {
          i += 2;
          continue;
        }
        i++;
        if (ch == _quote) break;
      }
      final end = i > src.length ? src.length : i;
      // 往后看一眼：跟着冒号就是键。
      var j = end;
      while (j < src.length && _isSpace(src.codeUnitAt(j))) {
        j++;
      }
      final isKey = j < src.length && src.codeUnitAt(j) == _colon;
      out.add(
        JsonToken(start, end, isKey ? JsonTokenKind.key : JsonTokenKind.string),
      );
      continue;
    }
    if (_isDigit(c) ||
        (c == 0x2d && i + 1 < src.length && _isDigit(src.codeUnitAt(i + 1)))) {
      final start = i;
      i++;
      while (i < src.length && _isNumberPart(src.codeUnitAt(i))) {
        i++;
      }
      out.add(JsonToken(start, i, JsonTokenKind.number));
      continue;
    }
    if (_isLetter(c)) {
      final start = i;
      while (i < src.length && _isLetter(src.codeUnitAt(i))) {
        i++;
      }
      final word = src.substring(start, i);
      out.add(
        JsonToken(
          start,
          i,
          const {'true', 'false', 'null'}.contains(word)
              ? JsonTokenKind.literal
              : JsonTokenKind.plain,
        ),
      );
      continue;
    }
    if (_isPunct(c)) {
      out.add(JsonToken(i, i + 1, JsonTokenKind.punct));
      i++;
      continue;
    }
    // 空白与其余字符：并进上一个 plain，或单独开一段。
    final start = i;
    while (i < src.length &&
        !_isPunct(src.codeUnitAt(i)) &&
        src.codeUnitAt(i) != _quote &&
        !_isLetter(src.codeUnitAt(i)) &&
        !_isDigit(src.codeUnitAt(i))) {
      i++;
    }
    if (i == start) i++;
    out.add(JsonToken(start, i, JsonTokenKind.plain));
  }
  return out;
}

bool _isSpace(int c) => c == 0x20 || c == 0x09 || c == 0x0a || c == 0x0d;
bool _isDigit(int c) => c >= 0x30 && c <= 0x39;
bool _isLetter(int c) =>
    (c >= 0x41 && c <= 0x5a) || (c >= 0x61 && c <= 0x7a) || c == 0x5f;
bool _isNumberPart(int c) =>
    _isDigit(c) ||
    c == 0x2e ||
    c == 0x65 ||
    c == 0x45 ||
    c == 0x2b ||
    c == 0x2d;
bool _isPunct(int c) =>
    c == 0x7b ||
    c == 0x7d ||
    c == 0x5b ||
    c == 0x5d ||
    c == 0x2c ||
    c == _colon;

// ── 上色 ──────────────────────────────────────────────────────────

/// 一套 JSON 配色。色值一律由调用方从主题取，本文件零硬编码颜色。
class JsonPalette {
  const JsonPalette({
    required this.key,
    required this.string,
    required this.number,
    required this.literal,
    required this.punct,
    required this.plain,
  });

  final Color key;
  final Color string;
  final Color number;
  final Color literal;
  final Color punct;
  final Color plain;

  Color of(JsonTokenKind k) => switch (k) {
    JsonTokenKind.key => key,
    JsonTokenKind.string => string,
    JsonTokenKind.number => number,
    JsonTokenKind.literal => literal,
    JsonTokenKind.punct => punct,
    JsonTokenKind.plain => plain,
  };
}

/// 带 JSON 语法高亮的输入控制器。
///
/// 高亮做在 `buildTextSpan` 里而不是另起一个渲染层：这样光标、选区、输入法
/// 预编辑全部照旧由 `TextField` 管，我们只改「这段文字用什么颜色画」。
class JsonHighlightController extends TextEditingController {
  JsonHighlightController({super.text, required this.palette});

  JsonPalette palette;

  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    final src = text;
    if (src.isEmpty) return TextSpan(style: style, text: src);
    final spans = <TextSpan>[];
    var cursor = 0;
    for (final tk in tokenizeJson(src)) {
      if (tk.start > cursor) {
        spans.add(
          TextSpan(text: src.substring(cursor, tk.start), style: style),
        );
      }
      spans.add(
        TextSpan(
          text: src.substring(tk.start, tk.end.clamp(0, src.length)),
          style: (style ?? const TextStyle()).copyWith(
            color: palette.of(tk.kind),
          ),
        ),
      );
      cursor = tk.end.clamp(0, src.length);
    }
    if (cursor < src.length) {
      spans.add(TextSpan(text: src.substring(cursor), style: style));
    }
    return TextSpan(style: style, children: spans);
  }
}
