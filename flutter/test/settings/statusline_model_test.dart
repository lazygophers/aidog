/// statusline 段模型的纯函数与数据表（票 I19b / C5）。
///
/// 期望值逐条对着 React 真值源核的：`statusline-segments.ts`（段定义 / 分类 /
/// 两套默认布局）、`statusline-gen.ts`（hexToRgb / groupRows / normalizeSegments /
/// isRowLeaderSeg / PREVIEW_METRIC）、`preview.tsx`（autoColorPreviewHex）、
/// `useStatusLinePanel.ts`（deleteRow / cycleRowAlign）。
///
/// Dart 的 Set/List 按身份比，断言一律先归一成字符串或标量。
library;

import 'package:aidog_flutter/shell.dart' show AidogColors;
import 'package:aidog_flutter/src/pages/settings/statusline_model.dart';
import 'package:aidog_flutter/utils/color_level.dart';
import 'package:flutter_test/flutter_test.dart';

StatusLineSegment seg(
  String id, {
  String type = 'model',
  bool enabled = true,
  bool newline = false,
  RowAlign? align,
  String? color,
  bool? autoColor,
  Map<String, Object?> options = const {},
}) => StatusLineSegment(
  id: id,
  type: type,
  enabled: enabled,
  newline: newline,
  align: align,
  color: color,
  autoColor: autoColor,
  options: options,
);

String ids(List<StatusLineSegment> l) => l.map((s) => s.id).join(',');

void main() {
  group('jsTruthy（JS 真值性，select 字段存的是字符串）', () {
    test('假值只有 null / false / 0 / NaN / 空串', () {
      expect(jsTruthy(null), isFalse);
      expect(jsTruthy(false), isFalse);
      expect(jsTruthy(0), isFalse);
      expect(jsTruthy(0.0), isFalse);
      expect(jsTruthy(double.nan), isFalse);
      expect(jsTruthy(''), isFalse);
    });

    test('字符串 "false" 是真 —— 与 React 一致，不是笔误', () {
      expect(jsTruthy('false'), isTrue);
      expect(jsTruthy('true'), isTrue);
      expect(jsTruthy(1), isTrue);
      expect(jsTruthy(-1), isTrue);
      expect(jsTruthy(<int>[]), isTrue);
    });
  });

  group('hexToRgb', () {
    test('6 位 / 3 位 / 带不带 # 都认', () {
      expect(hexToRgb('#4A9EFF')?.join(','), '74,158,255');
      expect(hexToRgb('4A9EFF')?.join(','), '74,158,255');
      expect(hexToRgb('#abc')?.join(','), '170,187,204');
      expect(hexToRgb('  #34C759  ')?.join(','), '52,199,89');
    });

    test('非法 / 空 → null', () {
      expect(hexToRgb(null), isNull);
      expect(hexToRgb(''), isNull);
      expect(hexToRgb('#12345'), isNull);
      expect(hexToRgb('#GGGGGG'), isNull);
      expect(hexToRgb('blue'), isNull);
    });
  });

  group('groupRows', () {
    test('按 newline 切行，行对齐取行首段的 align', () {
      final rows = groupRows([
        seg('a'),
        seg('b'),
        seg('c', newline: true, align: RowAlign.right),
        seg('d'),
      ]);
      expect(rows.length, 2);
      expect(ids(rows[0].segs), 'a,b');
      expect(rows[0].align, RowAlign.left);
      expect(ids(rows[1].segs), 'c,d');
      expect(rows[1].align, RowAlign.right);
    });

    test('首段带 newline 不会多切一个空行', () {
      final rows = groupRows([seg('a', newline: true), seg('b')]);
      expect(rows.length, 1);
      expect(ids(rows[0].segs), 'a,b');
    });

    test('空列表 → 零行', () {
      expect(groupRows(const []), isEmpty);
    });
  });

  group('normalizeSegments', () {
    test('第一段的 newline 被抹平，其余原样保留', () {
      final out = normalizeSegments([
        seg('a', newline: true),
        seg('b', newline: true),
        seg('c'),
      ]);
      expect(out[0].newline, isFalse);
      expect(out[1].newline, isTrue);
      expect(out[2].newline, isFalse);
    });

    test('空列表原样返回', () {
      expect(normalizeSegments(const []), isEmpty);
    });
  });

  group('isRowLeaderSeg', () {
    final list = [
      seg('a'),
      seg('b'),
      seg('c', newline: true),
      seg('off', enabled: false, newline: true),
      seg('off2', enabled: false),
    ];

    test('启用段：第一个或带 newline 的算行首', () {
      expect(isRowLeaderSeg(list, 'a'), isTrue);
      expect(isRowLeaderSeg(list, 'b'), isFalse);
      expect(isRowLeaderSeg(list, 'c'), isTrue);
    });

    test('停用段：只有显式 newline 才算行首', () {
      expect(isRowLeaderSeg(list, 'off'), isTrue);
      expect(isRowLeaderSeg(list, 'off2'), isFalse);
    });

    test('查不到的 id → false', () {
      expect(isRowLeaderSeg(list, 'nope'), isFalse);
    });
  });

  group('deleteRow', () {
    test('删掉整行（行归属按全量段推导，含停用段）', () {
      final list = [
        seg('a'),
        seg('b', enabled: false),
        seg('c', newline: true),
        seg('d'),
      ];
      expect(ids(deleteRow(list, 'a')), 'c,d');
      expect(ids(deleteRow(list, 'c')), 'a,b');
    });

    test('传行内非行首段，删的也是整行', () {
      final list = [seg('a'), seg('b'), seg('c', newline: true)];
      expect(ids(deleteRow(list, 'b')), 'c');
    });

    test('id 不存在 → 原样返回', () {
      final list = [seg('a')];
      expect(identical(deleteRow(list, 'zz'), list), isTrue);
    });
  });

  group('cycleRowAlign', () {
    test('left → center → right → left，写在行首段上', () {
      var list = [seg('a'), seg('b')];
      list = cycleRowAlign(list, 'b'); // 从 b 往回找到行首 a
      expect(list[0].align, RowAlign.center);
      expect(list[1].align, isNull);
      list = cycleRowAlign(list, 'a');
      expect(list[0].align, RowAlign.right);
      list = cycleRowAlign(list, 'a');
      expect(list[0].align, RowAlign.left);
    });

    test('只改本行行首，不碰别的行', () {
      final list = cycleRowAlign([
        seg('a'),
        seg('b', newline: true),
        seg('c'),
      ], 'c');
      expect(list[0].align, isNull);
      expect(list[1].align, RowAlign.center);
    });

    test('停用段 / 未知 id → 原样返回', () {
      final list = [seg('a'), seg('x', enabled: false)];
      expect(identical(cycleRowAlign(list, 'x'), list), isTrue);
      expect(identical(cycleRowAlign(list, 'nope'), list), isTrue);
    });
  });

  group('autoColorPreviewLevel（阈值照抄 bash）', () {
    test('cost 类：模拟值 12 分 → success', () {
      expect(autoColorPreviewLevel('cost'), ColorLevel.success);
      expect(autoColorPreviewLevel('cost-usd'), ColorLevel.success);
    });

    test('context-remaining 49% → success；rate-limit-7d 62 → warning', () {
      expect(autoColorPreviewLevel('context-remaining'), ColorLevel.success);
      expect(autoColorPreviewLevel('rate-limit-7d'), ColorLevel.warning);
    });

    test('session-duration 285s → warning；api-duration 15s → success', () {
      expect(autoColorPreviewLevel('session-duration'), ColorLevel.warning);
      expect(autoColorPreviewLevel('api-duration'), ColorLevel.success);
    });

    test('通用分支：context-pct 65 → warning；未登记的类型按 0 → success', () {
      expect(autoColorPreviewLevel('context-pct'), ColorLevel.warning);
      expect(autoColorPreviewLevel('context-bar'), ColorLevel.warning);
      expect(autoColorPreviewLevel('rate-limits'), ColorLevel.success);
      expect(autoColorPreviewLevel('model'), ColorLevel.success);
    });
  });

  group('previewColor', () {
    const c = AidogColors.dark;

    test('autoColor 且是值型段 → 分级色', () {
      final s = seg('a', type: 'rate-limit-7d', autoColor: true);
      expect(previewColor(s, c), levelColor(ColorLevel.warning, c));
    });

    test('autoColor 但不是值型段 → 回落固定色', () {
      final s = seg('a', type: 'model', autoColor: true, color: '#4A9EFF');
      expect(previewColor(s, c)?.toARGB32(), 0xFF4A9EFF);
    });

    test('无 autoColor 无合法色 → null', () {
      expect(previewColor(seg('a'), c), isNull);
      expect(previewColor(seg('a', color: 'nope'), c), isNull);
    });
  });

  group('makeSegment', () {
    test('带上定义表里的默认选项', () {
      final s = makeSegment('context-bar', id: 's1')!;
      expect(s.id, 's1');
      expect(s.enabled, isTrue);
      expect(s.newline, isFalse);
      expect(s.options['width'], 10);
      expect(s.options['filled'], '▓');
    });

    test('newline 可指定；未知类型 → null', () {
      expect(makeSegment('model', newline: true, id: 'x')!.newline, isTrue);
      expect(makeSegment('no-such-type', id: 'x'), isNull);
    });

    test('返回的 options 是副本，改它不污染定义表', () {
      final s = makeSegment('model', id: 'a')!;
      s.options['format'] = 'full';
      expect(kSegmentDefMap['model']!.defaultOptions['format'], 'short');
    });
  });

  group('StatusLineSegment 序列化', () {
    test('toJson 不写 null 字段（与 React 的 undefined 同效）', () {
      final j = seg('a').toJson();
      expect(j.containsKey('color'), isFalse);
      expect(j.containsKey('autoColor'), isFalse);
      expect(j.containsKey('align'), isFalse);
    });

    test('有值时写出来，align 落字符串', () {
      final j = seg(
        'a',
        color: '#fff',
        autoColor: true,
        align: RowAlign.center,
      ).toJson();
      expect(j['color'], '#fff');
      expect(j['autoColor'], true);
      expect(j['align'], 'center');
    });

    test('fromJson 往返稳定；缺字段有兜底', () {
      final s = StatusLineSegment.fromJson(seg(
        'a',
        type: 'cost',
        newline: true,
        align: RowAlign.right,
        options: const {'showDuration': false},
      ).toJson());
      expect(s.id, 'a');
      expect(s.type, 'cost');
      expect(s.newline, isTrue);
      expect(s.align, RowAlign.right);
      expect(s.options['showDuration'], false);

      final bare = StatusLineSegment.fromJson({'id': 'b', 'type': 'model'});
      expect(bare.enabled, isFalse);
      expect(bare.newline, isFalse);
      expect(bare.options, isEmpty);
      expect(bare.align, isNull);
    });

    test('rowAlignFrom 认三个值，其余 null', () {
      expect(rowAlignFrom('left'), RowAlign.left);
      expect(rowAlignFrom('center'), RowAlign.center);
      expect(rowAlignFrom('right'), RowAlign.right);
      expect(rowAlignFrom('middle'), isNull);
      expect(rowAlignFrom(null), isNull);
    });

    test('copyWith 的 clear* 能把字段抹回 null', () {
      final s = seg('a', color: '#fff', autoColor: true, align: RowAlign.right);
      final cleared = s.copyWith(
        clearColor: true,
        clearAutoColor: true,
        clearAlign: true,
      );
      expect(cleared.color, isNull);
      expect(cleared.autoColor, isNull);
      expect(cleared.align, isNull);
      expect(cleared.id, 'a');
    });
  });

  group('段定义表', () {
    test('类型唯一，分类里列到的类型都存在', () {
      final types = kSegmentDefs.map((d) => d.type).toList();
      expect(types.length, types.toSet().length);
      final catTypes = [
        for (final c in kSegmentCategories) ...c.types,
      ];
      expect(catTypes.where((t) => !kSegmentDefMap.containsKey(t)), isEmpty);
      // 分类表覆盖的 = group-* 之外的全部段（group-* 不在选择器里，与 React 一致）。
      final uncategorized = types.where((t) => !catTypes.contains(t)).toSet();
      expect(uncategorized.difference(kGroupSegTypes), isEmpty);
    });

    test('两套默认布局引用的段类型都在表里', () {
      for (final s in [...kDefaultSegments, ...kDefaultSubagentSegments]) {
        expect(kSegmentDefMap.containsKey(s.type), isTrue, reason: s.type);
      }
    });

    test('默认布局首段不带 newline（normalizeSegments 的不变量）', () {
      expect(kDefaultSegments.first.newline, isFalse);
      expect(kDefaultSubagentSegments.first.newline, isFalse);
    });

    test('主布局切 3 行，子代理布局 1 行', () {
      expect(groupRows(kDefaultSegments).length, 3);
      expect(groupRows(kDefaultSubagentSegments).length, 1);
    });

    test('每个段的 toPreview 在「默认选项」下都出非空串', () {
      for (final d in kSegmentDefs) {
        expect(d.toPreview(d.defaultOptions), isNotEmpty, reason: d.type);
      }
    });

    test('toPreview 在「空选项」下也不炸（用户删光了选项键）', () {
      for (final d in kSegmentDefs) {
        expect(() => d.toPreview(const {}), returnsNormally, reason: d.type);
      }
    });
  });

  group('toPreview 的逐条期望值（照抄 React 字面量）', () {
    String p(String type, [Map<String, Object?> o = const {}]) {
      final d = kSegmentDefMap[type]!;
      return d.toPreview({...d.defaultOptions, ...o});
    }

    test('model', () {
      expect(p('model'), 'Opus');
      expect(p('model', {'format': 'full'}), 'claude-sonnet-4-6');
    });

    test('context-bar：宽度 / 填充字符可改，0 宽回落 10', () {
      expect(p('context-bar'), '▓▓▓▓▓▓▓░░░ 65%');
      expect(p('context-bar', {'width': 4}), '▓▓▓░ 65%');
      expect(p('context-bar', {'width': 0}), '▓▓▓▓▓▓▓░░░ 65%');
      expect(p('context-bar', {'filled': '#', 'empty': '-'}), '#######--- 65%');
      // 空串是 falsy → 回落默认字符（React 用的是 `||`）。
      expect(p('context-bar', {'filled': ''}), '▓▓▓▓▓▓▓░░░ 65%');
    });

    test('git / cost：select 存的 "false" 是真值', () {
      expect(p('git'), 'claude-code');
      expect(p('git', {'showRepo': 'false'}), 'anthropics/claude-code');
      expect(p('cost'), r'$0.12 · 155s');
      expect(p('cost', {'showDuration': false}), r'$0.12');
    });

    test('rate-limits 三挡', () {
      expect(p('rate-limits'), '5h:23% 7d:41%');
      expect(p('rate-limits', {'windows': '5h'}), '5h:23%');
      expect(p('rate-limits', {'windows': '7d'}), '7d:41%');
    });

    test('separator：非字符串回落 ·', () {
      expect(p('separator'), '·');
      expect(p('separator', {'char': ' | '}), ' | ');
      expect(p('separator', {'char': 3}), '·');
    });

    test('前缀型段用 ?? 回落，空串前缀照用', () {
      expect(p('group-balance'), '余额 48.20');
      expect(p('group-balance', {'prefix': ''}), '48.20');
      expect(p('cost-usd'), r'$0.12');
      expect(p('pr-number'), '#123');
      expect(p('version'), 'v2.1.90');
      expect(p('group-tokens'), '1.2M');
      expect(p('group-window-cost'), '折算\$4.56');
    });

    test('context-tokens 四种组合', () {
      expect(p('context-tokens'), '89.5K/12.4K');
      expect(p('context-tokens', {'abbrev': false}), '89500/12400');
      expect(p('context-tokens', {'mode': 'sum'}), '101.9K');
      expect(p('context-tokens', {'mode': 'sum', 'abbrev': false}), '101900');
    });

    test('context-cache 两模式', () {
      expect(p('context-cache'), 'w20K r12.1K');
      expect(p('context-cache', {'abbrev': false}), 'w20000 r12100');
      expect(p('context-cache', {'mode': 'hitrate'}), '缓存 13.3578%');
    });

    test('时长 / 限制 / 会话 id 的两态', () {
      expect(p('session-duration'), '4m45s');
      expect(p('session-duration', {'format': 'ms'}), '285000ms');
      expect(p('api-duration'), '15s');
      expect(p('api-duration', {'format': 'ms'}), '15300ms');
      expect(p('rate-limit-5h'), '5h:34%');
      expect(p('rate-limit-5h', {'showReset': true}), '5h:34% (128m)');
      expect(p('rate-limit-7d', {'showReset': true}), '7d:62% (40h)');
      expect(p('session-id'), 'abc123xy');
      expect(p('session-id', {'truncate': false}), 'abc123xyz789');
    });

    test('路径型段的 basename / full', () {
      expect(p('cwd'), 'aidog');
      expect(p('cwd', {'format': 'full'}), '/Users/luoxin/persons/aidog');
      expect(p('project-dir', {'format': 'full'}), '/Users/luoxin/persons/aidog');
      expect(p('transcript-path'), 'session.jsonl');
      expect(
        p('transcript-path', {'format': 'full'}),
        '/Users/luoxin/.claude/session.jsonl',
      );
    });

    test('文案型段与 custom', () {
      expect(p('thinking'), 'thinking');
      expect(p('thinking', {'label': '🧠'}), '🧠');
      expect(p('token-warn'), '⚠200k');
      expect(p('custom'), '<.model.display_name>');
      expect(p('custom', {'expr': '.x'}), '<.x>');
      // 空串是 falsy → 回落默认表达式（React 用的是 `||`）。
      expect(p('custom', {'expr': ''}), '<.model.display_name>');
    });

    test('固定文本段', () {
      expect(p('context-pct'), '65%');
      expect(p('context-max'), '200K');
      expect(p('context-max', {'abbrev': false}), '200000');
      expect(p('context-remaining'), '49%');
      expect(p('lines-changed'), '+412 -87');
      expect(p('effort'), 'high');
      expect(p('vim'), 'NORMAL');
      expect(p('git-branch'), 'main');
      expect(p('git-repo-full'), 'anthropics/claude-code');
      expect(p('agent-badge'), '[Agent·●·Opus]');
      expect(p('group-route'), 'glm/GLM-自用');
      expect(p('group-coding'), '5h 23%·7d 41%');
      expect(p('group-requests'), '128·99%');
      expect(p('group-cache'), '缓存 37%');
      expect(p('group-spent'), r'$1.23');
    });
  });

  test('effectiveOptions = 定义默认 + 用户覆盖', () {
    final d = kSegmentDefMap['context-bar']!;
    final o = effectiveOptions(d, seg('a', options: const {'width': 3}));
    expect(o['width'], 3);
    expect(o['filled'], '▓');
  });

  test('数据字段参考表：7 组、id 唯一', () {
    expect(kStatuslineDataFields.length, 7);
    final idList = kStatuslineDataFields.map((g) => g.id).toList();
    expect(idList.length, idList.toSet().length);
    expect(idList.join(','),
        'model,workspace,cost,contextWindow,rateLimits,other,subagent');
  });
}
