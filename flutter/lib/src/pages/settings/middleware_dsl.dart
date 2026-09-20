/// 中间件条件树 DSL（I17）—— 逐行移植 `src/utils/mwDsl.ts`（票 05）。
///
/// 树 JSON 是唯一存储真值；DSL 只是视图。语法（S 表达式风格，括号内换行随意）：
///
///   ALL( 叶子 叶子 ... )   ANY( 叶子 ... )   NOT( 单个子条件 )   叶子
///   叶子 := target[.field] OP "pattern" [checksum 校验器名]
///   OP := contains | regex | exact；校验器 := luhn | iban | cn_id
///
/// 往返保证：tree → dsl → tree 与原树深度相等（叶子顺序保持）。
/// 树的形状与后端 serde 一致：leaf = {kind,target,field,match_type,pattern,validator}，
/// all/any = {kind,children:[…]}，not = {kind,child:…}。
library;

import 'dart:convert';

const List<String> kDslTargets = [
  'request_body', 'request_headers', 'response_body', 'response_headers',
  'status', 'model',
];
const List<String> kDslOps = ['contains', 'regex', 'exact'];
const List<String> kDslValidators = ['luhn', 'iban', 'cn_id'];

/// DSL 解析 / 拼装错误，带字符位置（供编辑器提示「位置 N」）。
class DslException implements Exception {
  DslException(this.pos, this.message);
  final int pos;
  final String message;

  @override
  String toString() => '位置 ${pos + 1}: $message';
}

/// 叶子 / 树 → DSL。pattern 用 JSON 字符串转义（可含任意字符）。
String treeToDsl(Map<String, Object?> node) {
  final kind = '${node['kind']}';
  if (kind == 'leaf') {
    final field = '${node['field'] ?? ''}'.isEmpty ? '' : '.${node['field']}';
    final checksum =
        '${node['validator'] ?? ''}'.isEmpty ? '' : ' checksum ${node['validator']}';
    return '${node['target']}$field ${node['match_type']} '
        '${jsonEncode(node['pattern'])}$checksum';
  }
  if (kind == 'not') {
    return 'NOT(\n  ${treeToDsl(Map<String, Object?>.from(node['child'] as Map))}\n)';
  }
  final inner = [
    for (final c in (node['children'] as List? ?? const []))
      treeToDsl(Map<String, Object?>.from(c as Map)),
  ];
  if (inner.length == 1) return inner[0];
  return '${kind.toUpperCase()}(\n  ${inner.join('\n  ')}\n)';
}

// ── 词法 ────────────────────────────────────────────────

class _Tok {
  const _Tok.word(this.v) : isStr = false, isLParen = false, isRParen = false;
  const _Tok.str(this.v) : isStr = true, isLParen = false, isRParen = false;
  const _Tok.lParen()
      : v = '',
        isStr = false,
        isLParen = true,
        isRParen = false;
  const _Tok.rParen()
      : v = '',
        isStr = false,
        isLParen = false,
        isRParen = true;
  final String v;
  final bool isStr;
  final bool isLParen;
  final bool isRParen;
}

class _Lexer {
  _Lexer(this.s);
  final String s;
  int i = 0;

  ({_Tok tok, int pos}) next() {
    while (i < s.length && RegExp(r'\s').hasMatch(s[i])) {
      i++;
    }
    final pos = i;
    if (i >= s.length) return (tok: const _Tok.word(''), pos: pos); // eof
    final c = s[i];
    if (c == '(') {
      i++;
      return (tok: const _Tok.lParen(), pos: pos);
    }
    if (c == ')') {
      i++;
      return (tok: const _Tok.rParen(), pos: pos);
    }
    if (c == '"') {
      var j = ++i;
      while (j < s.length && s[j] != '"') {
        if (s[j] == '\\') j++;
        j++;
      }
      if (j >= s.length) throw DslException(pos, '未闭合的字符串');
      String v;
      try {
        v = jsonDecode(s.substring(pos, j + 1)) as String;
      } catch (_) {
        throw DslException(pos, '非法字符串字面量');
      }
      i = j + 1;
      return (tok: _Tok.str(v), pos: pos);
    }
    final m = RegExp(r'[A-Za-z0-9_.\-]+').matchAsPrefix(s, i);
    if (m == null) throw DslException(pos, "非法字符 '$c'");
    i = m.end;
    return (tok: _Tok.word(m[0]!), pos: pos);
  }
}

// ── 语法 ────────────────────────────────────────────────

class _Parser {
  _Parser(this._lx);
  final _Lexer _lx;
  ({_Tok tok, int pos})? _la;

  ({_Tok tok, int pos}) peek() => _la ??= _lx.next();

  _Tok take() {
    final p = peek();
    _la = null;
    return p.tok;
  }

  Map<String, Object?> parseExpr() {
    final p = peek();
    if (p.tok.v == 'NOT' && !p.tok.isStr) {
      take();
      if (!take().isLParen) throw DslException(p.pos, "NOT 后缺 '('");
      final child = parseExpr();
      if (!take().isRParen) {
        throw DslException(p.pos, 'NOT() 只接受一个子条件');
      }
      return {'kind': 'not', 'child': child};
    }
    if ((p.tok.v == 'ALL' || p.tok.v == 'ANY') && !p.tok.isStr) {
      take();
      if (!take().isLParen) throw DslException(p.pos, "${p.tok.v} 后缺 '('");
      final children = <Map<String, Object?>>[];
      while (true) {
        final n = peek();
        if (n.tok.v.isEmpty && !n.tok.isStr && !n.tok.isLParen && !n.tok.isRParen) {
          throw DslException(n.pos, "缺 ')'");
        }
        if (n.tok.isRParen) {
          take();
          break;
        }
        children.add(parseExpr());
      }
      if (children.isEmpty) {
        throw DslException(p.pos, '${p.tok.v}() 至少需要一个子条件');
      }
      return {'kind': p.tok.v.toLowerCase(), 'children': children};
    }
    return parseLeaf();
  }

  Map<String, Object?> parseLeaf() {
    final p = peek();
    if (p.tok.isStr || p.tok.isLParen || p.tok.isRParen ||
        (p.tok.v.isEmpty && !p.tok.isStr)) {
      throw DslException(p.pos, '期望条件（target op "pattern"）');
    }
    take();
    final segs = p.tok.v.split('.');
    final target = segs[0];
    final field = segs.sublist(1).join('.');
    if (!kDslTargets.contains(target)) {
      throw DslException(p.pos, "未知 target '$target'（可选: ${kDslTargets.join(' / ')}）");
    }
    final opTok = take();
    if (opTok.isStr || opTok.isLParen || opTok.isRParen ||
        !kDslOps.contains(opTok.v)) {
      throw DslException(p.pos, '期望算子（${kDslOps.join(' / ')}）');
    }
    final patTok = take();
    if (!patTok.isStr) throw DslException(p.pos, '期望双引号 pattern');
    var validator = '';
    final after = peek();
    if (after.tok.v == 'checksum' && !after.tok.isStr) {
      take();
      final nameTok = take();
      if (nameTok.isStr || nameTok.isLParen || nameTok.isRParen ||
          nameTok.v.isEmpty) {
        throw DslException(after.pos, 'checksum 后缺校验器名');
      }
      if (!kDslValidators.contains(nameTok.v)) {
        throw DslException(
          after.pos,
          "未知校验器 '${nameTok.v}'（可选: ${kDslValidators.join(' / ')}）",
        );
      }
      validator = nameTok.v;
    }
    return {
      'kind': 'leaf',
      'target': target,
      'field': field,
      'match_type': opTok.v,
      'pattern': patTok.v,
      'validator': validator,
    };
  }
}

/// DSL → 树。解析失败抛 [DslException]。
Map<String, Object?> parseDsl(String src) {
  final parser = _Parser(_Lexer(src));
  final tree = parser.parseExpr();
  final rest = parser.peek();
  if (rest.tok.v.isNotEmpty || rest.tok.isStr || rest.tok.isLParen || rest.tok.isRParen) {
    throw DslException(rest.pos, '条件后有多余内容');
  }
  return tree;
}

/// target 是否响应侧（与 Rust `Target::is_response_side` 对称）。
bool isResponseTarget(String t) =>
    t == 'response_body' || t == 'response_headers' || t == 'status';

/// 混阶段检查：树内所有叶子必须同侧（与 Rust validate_rule_phases 对称，
/// 前端提前提示）。混了返回错误文案，合法返回 null。
String? mixedPhase(Map<String, Object?> node) {
  bool? phase;
  try {
    _walk(node, (leaf) {
      final p = isResponseTarget('${leaf['target']}');
      if (phase != null && phase != p) {
        throw StateError(
          "混阶段条件被拒：'${leaf['target']}' 与请求侧条件不能同树",
        );
      }
      phase = p;
    });
    return null;
  } on StateError catch (e) {
    return e.message;
  }
}

void _walk(
  Map<String, Object?> node,
  void Function(Map<String, Object?> leaf) visit,
) {
  final kind = '${node['kind']}';
  if (kind == 'leaf') {
    visit(node);
  } else if (kind == 'not') {
    _walk(Map<String, Object?>.from(node['child'] as Map), visit);
  } else {
    for (final c in (node['children'] as List? ?? const [])) {
      _walk(Map<String, Object?>.from(c as Map), visit);
    }
  }
}
