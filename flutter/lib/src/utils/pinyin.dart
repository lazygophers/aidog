/// 拼音模糊搜索 —— 对齐 React 版 `src/utils/pinyin.ts::pinyinMatch` 的匹配语义。
///
/// 词典是 `pinyin_data.dart` 里的 3500 常用字（自建，见
/// `scripts/gen-flutter-pinyin-data.mjs` 的文件头说明为什么不引 pub.dev 包）。
/// 匹配语义照抄 pinyin-pro 的四条链：
/// ① 直接子串 ② 目标转全拼后子串 ③ 查询里的中文也转全拼再比
/// ④ 纯拉丁 query 打首字母串（如 `fz` → 「分组」、`yzam` → 「月之暗面」）。
///
/// 覆盖范围：3500 字外的生僻字按「原字符保留」处理（与 pinyin-pro 查不到时
/// 返回原字符的行为一致），不会报错、只可能搜不到。
///
/// 转换结果按 target 缓存（React 侧是 500 条 LRU；这里列表规模有限，直接
/// Map 不淘汰，上限 = 用户平台 / 分组名的去重数）。
library;

import 'pinyin_data.dart';

/// CJK 基本区（React 侧同一个正则 `/[一-鿿]/`）。
final RegExp _cjk = RegExp(r'[一-鿿]');

Map<String, String> _pyOf = {};
Map<String, String> _initialsOf = {};

/// 字 → 全拼 / 首字母表，按需建一次。
Map<String, int> _index = {};
List<String> _full = [];
bool _built = false;

void _ensureTable() {
  if (_built) return;
  _full = kPinyinFull.split(' ');
  for (var i = 0; i < kPinyinChars.length; i++) {
    _index[kPinyinChars[i]] = i;
  }
  _built = true;
}

/// 字符串里的中文转全拼（无声调，小写），非中文字符原样保留。
/// 词典未覆盖的汉字也原样保留（对齐 pinyin-pro 查不到时的行为）。
String toPinyin(String text) =>
    _pyOf.putIfAbsent(text, () => _convert(text, full: true));

/// 中文字符的首字母串（「百炼」→ `bl`）。非中文字符跳过（不保留）。
String toInitials(String text) =>
    _initialsOf.putIfAbsent(text, () => _convert(text, full: false));

String _convert(String text, {required bool full}) {
  _ensureTable();
  final out = StringBuffer();
  for (final ch in text.runes) {
    final s = String.fromCharCode(ch);
    final idx = _cjk.hasMatch(s) ? (_index[s] ?? -1) : -1;
    if (idx >= 0) {
      out.write(full ? _full[idx] : kPinyinInitials[idx]);
    } else if (full) {
      out.write(s);
    }
  }
  return out.toString().toLowerCase();
}

/// 拼音模糊匹配：空查询显示全部；支持纯拼音 / 首字母 / 中文 / 中英混合。
bool pinyinMatch(String query, String target) {
  final q = query.toLowerCase().trim();
  if (q.isEmpty) return true;

  final t = target.toLowerCase();

  // 1. 直接子串
  if (t.contains(q)) return true;

  // 2. 目标转拼音后匹配
  final targetPinyin = toPinyin(target);
  if (targetPinyin.contains(q)) return true;

  // 3. 查询中的中文也转拼音再匹配
  final queryPinyin = toPinyin(query);
  if (targetPinyin.contains(queryPinyin)) return true;
  if (t.contains(queryPinyin)) return true;

  // 4. 拼音首字母匹配（query 为纯拉丁时）
  if (!_cjk.hasMatch(q)) {
    if (toInitials(target).contains(q)) return true;
  }

  return false;
}
