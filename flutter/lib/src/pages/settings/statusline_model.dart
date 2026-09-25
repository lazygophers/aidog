/// statusline 段模型 + 数据表（纯数据，零 widget 依赖）。
///
/// 真值源逐条照抄自 React：
/// - `src/components/settings/statusline-segments.ts`（段定义表 / 分类 / 两套默认布局）
/// - `src/components/settings/statusline-gen.ts`（hexToRgb / groupRows /
///   normalizeSegments / isRowLeaderSeg / PREVIEW_METRIC）
/// - `src/components/settings/editors/StatusLineSection/preview.tsx`
///   （autoColorPreviewHex / previewColor）
/// - `src/components/settings/editors/StatusLineSection/useStatusLinePanel.ts`
///   （deleteRow / cycleRowAlign / addSegment 的纯计算部分外提到这里）
///
/// 🔴 `toPreview` 里的真假判断照搬的是 **JS 真值性**，不是 Dart 的 bool：
/// select 型字段存的是字符串 `"false"`，JS 里它是 truthy。见 [jsTruthy]。
/// 这不是笔误 —— 与 React 行为不同就是 bug（票 I18 的对齐口径）。
library;

import 'package:flutter/widgets.dart';

import '../../../utils/color_level.dart';
import '../../../utils/hex_color.dart';

// hexToRgb 原先住在本文件，平台卡的协议品牌色也要用它，故提到 `utils/hex_color.dart`
// 只留一份。这里转发，原有调用方与测试不必各加一行 import。
export '../../../utils/hex_color.dart' show hexToRgb, parseHexColor;
import '../../shell/theme.dart';

/// 行对齐。
enum RowAlign { left, center, right }

RowAlign? rowAlignFrom(Object? v) => switch ('$v') {
  'left' => RowAlign.left,
  'center' => RowAlign.center,
  'right' => RowAlign.right,
  _ => null,
};

String rowAlignName(RowAlign a) => a.name;

/// JS 的真值性：`null` / `false` / `0` / `""` / `NaN` 为假，其余为真。
///
/// 字符串 `"false"` 在 JS 里是**真**。React 的段选项里 select 型字段存的正是
/// `"true"` / `"false"` 字符串，所以这条必须照搬。
bool jsTruthy(Object? v) {
  if (v == null) return false;
  if (v is bool) return v;
  if (v is num) return v != 0 && !v.isNaN;
  if (v is String) return v.isNotEmpty;
  return true;
}

/// 一个段。字段名照抄 `statusline-segments.ts::StatusLineSegment`。
class StatusLineSegment {
  const StatusLineSegment({
    required this.id,
    required this.type,
    required this.enabled,
    required this.newline,
    this.options = const {},
    this.color,
    this.autoColor,
    this.align,
  });

  final String id;
  final String type;
  final bool enabled;

  /// 在本段前插入换行（为真时本段是行首）。
  final bool newline;
  final Map<String, Object?> options;

  /// 固定前景色（`#RRGGBB` / `#RGB`）。
  final String? color;

  /// 值型段：按阈值自动上色。
  final bool? autoColor;

  /// 行对齐 —— 只有行首段上的这个字段有意义。
  final RowAlign? align;

  StatusLineSegment copyWith({
    String? id,
    String? type,
    bool? enabled,
    bool? newline,
    Map<String, Object?>? options,
    String? color,
    bool clearColor = false,
    bool? autoColor,
    bool clearAutoColor = false,
    RowAlign? align,
    bool clearAlign = false,
  }) => StatusLineSegment(
    id: id ?? this.id,
    type: type ?? this.type,
    enabled: enabled ?? this.enabled,
    newline: newline ?? this.newline,
    options: options ?? this.options,
    color: clearColor ? null : (color ?? this.color),
    autoColor: clearAutoColor ? null : (autoColor ?? this.autoColor),
    align: clearAlign ? null : (align ?? this.align),
  );

  static StatusLineSegment fromJson(Map<String, Object?> j) =>
      StatusLineSegment(
        id: '${j['id']}',
        type: '${j['type']}',
        enabled: j['enabled'] == true,
        newline: j['newline'] == true,
        options: j['options'] is Map
            ? Map<String, Object?>.from(j['options']! as Map)
            : const {},
        color: j['color'] as String?,
        autoColor: j['autoColor'] as bool?,
        align: rowAlignFrom(j['align']),
      );

  /// 回写形状与 React 一致：`undefined` 的键不落盘（Dart 这边就是不写这个键）。
  Map<String, Object?> toJson() => {
    'id': id,
    'type': type,
    'enabled': enabled,
    'newline': newline,
    'options': options,
    if (color != null) 'color': color,
    if (autoColor != null) 'autoColor': autoColor,
    if (align != null) 'align': rowAlignName(align!),
  };
}

/// 段的可编辑字段。
class SegmentField {
  const SegmentField({
    required this.key,
    required this.label,
    required this.type,
    this.options = const [],
    this.placeholder,
  });

  final String key;
  final String label;

  /// `string` / `number` / `select`
  final String type;
  final List<String> options;
  final String? placeholder;
}

/// 段定义。
class SegmentDef {
  const SegmentDef({
    required this.type,
    required this.name,
    required this.desc,
    this.defaultOptions = const {},
    required this.toPreview,
    this.fields = const [],
  });

  final String type;
  final String name;
  final String desc;
  final Map<String, Object?> defaultOptions;
  final String Function(Map<String, Object?> opts) toPreview;
  final List<SegmentField> fields;
}

/// 值可驱动自动上色的段类型。
const kValueColorable = <String>{
  'context-pct',
  'context-bar',
  'cost',
  'rate-limits',
  'cost-usd',
  'context-remaining',
  'rate-limit-5h',
  'rate-limit-7d',
  'session-duration',
  'api-duration',
};

/// 消费 aidog group-info 端点的段类型。
const kGroupSegTypes = <String>{
  'group-balance',
  'group-spent',
  'group-window-cost',
  'group-coding',
  'group-requests',
  'group-cache',
  'group-tokens',
  'group-route',
};

String _str(Object? v, String fallback) =>
    v is String ? v : (v == null ? fallback : '$v');

/// `o.x ?? fallback`（只在 null 时回落，空串照用）。
String _nullish(Map<String, Object?> o, String key, String fallback) {
  final v = o[key];
  return v == null ? fallback : _str(v, fallback);
}

/// `o.x || fallback`（JS falsy 时回落 —— 空串也回落）。
String _falsy(Map<String, Object?> o, String key, String fallback) {
  final v = o[key];
  return jsTruthy(v) ? _str(v, fallback) : fallback;
}

final List<SegmentDef> kSegmentDefs = [
  SegmentDef(
    type: 'model',
    name: '模型名称',
    desc: '当前模型显示名称',
    defaultOptions: const {'format': 'short'},
    toPreview: (o) => o['format'] == 'full' ? 'claude-sonnet-4-6' : 'Opus',
    fields: const [
      SegmentField(
        key: 'format',
        label: '格式',
        type: 'select',
        options: ['short', 'full'],
      ),
    ],
  ),
  SegmentDef(
    type: 'context-bar',
    name: '上下文进度条',
    desc: '10 字符进度条 + 百分比',
    defaultOptions: const {'width': 10, 'filled': '▓', 'empty': '░'},
    toPreview: (o) {
      final wRaw = o['width'];
      final w = (wRaw is num && wRaw != 0) ? wRaw.round() : 10;
      const pct = 65;
      final filled = (pct * w / 100).round();
      final bar =
          _falsy(o, 'filled', '▓') * filled +
          _falsy(o, 'empty', '░') * (w - filled);
      return '$bar $pct%';
    },
    fields: const [
      SegmentField(
        key: 'width',
        label: '宽度',
        type: 'number',
        placeholder: '10',
      ),
      SegmentField(
        key: 'filled',
        label: '填充字符',
        type: 'string',
        placeholder: '▓',
      ),
      SegmentField(
        key: 'empty',
        label: '空字符',
        type: 'string',
        placeholder: '░',
      ),
    ],
  ),
  SegmentDef(
    type: 'context-pct',
    name: '上下文百分比',
    desc: '仅百分比数字',
    defaultOptions: const {'suffix': '%'},
    toPreview: (o) => '65%',
  ),
  SegmentDef(
    type: 'git',
    name: 'Git 状态',
    desc: '分支名 + 仓库名',
    defaultOptions: const {'showRepo': false},
    toPreview: (o) =>
        jsTruthy(o['showRepo']) ? 'anthropics/claude-code' : 'claude-code',
    fields: const [
      SegmentField(
        key: 'showRepo',
        label: '显示完整路径 (owner/name)',
        type: 'select',
        options: ['false', 'true'],
      ),
    ],
  ),
  SegmentDef(
    type: 'cost',
    name: '成本追踪',
    desc: 'API 成本 + 持续时间',
    defaultOptions: const {'showDuration': true},
    toPreview: (o) => jsTruthy(o['showDuration']) ? r'$0.12 · 155s' : r'$0.12',
    fields: const [
      SegmentField(
        key: 'showDuration',
        label: '显示持续时间',
        type: 'select',
        options: ['true', 'false'],
      ),
    ],
  ),
  SegmentDef(
    type: 'rate-limits',
    name: '速率限制',
    desc: '5h / 7d 限制使用百分比',
    defaultOptions: const {'windows': 'both'},
    toPreview: (o) => o['windows'] == '5h'
        ? '5h:23%'
        : o['windows'] == '7d'
        ? '7d:41%'
        : '5h:23% 7d:41%',
    fields: const [
      SegmentField(
        key: 'windows',
        label: '窗口',
        type: 'select',
        options: ['both', '5h', '7d'],
      ),
    ],
  ),
  SegmentDef(
    type: 'effort',
    name: 'Effort Level',
    desc: '推理工作量等级',
    toPreview: (o) => 'high',
  ),
  SegmentDef(
    type: 'vim',
    name: 'Vim 模式',
    desc: '当前 vim 模式',
    toPreview: (o) => 'NORMAL',
  ),
  SegmentDef(
    type: 'separator',
    name: '分隔符',
    desc: '视觉分隔符（可插入到任意段之间）',
    defaultOptions: const {'char': '·'},
    toPreview: (o) => o['char'] is String ? o['char']! as String : '·',
    fields: const [
      SegmentField(
        key: 'char',
        label: '分隔符字符',
        type: 'string',
        placeholder: '·',
      ),
    ],
  ),
  SegmentDef(
    type: 'group-balance',
    name: '分组余额',
    desc: '当前分组单启用平台预估剩余余额（动态色：<1天红 / <3天黄 / 否则绿）',
    defaultOptions: const {'prefix': '余额 ', 'dynamicColor': false},
    toPreview: (o) => '${_nullish(o, 'prefix', '余额 ')}48.20',
    fields: const [
      SegmentField(
        key: 'prefix',
        label: '前缀',
        type: 'string',
        placeholder: '余额 ',
      ),
      SegmentField(
        key: 'dynamicColor',
        label: '动态色 (按可用天数)',
        type: 'select',
        options: ['false', 'true'],
      ),
    ],
  ),
  SegmentDef(
    type: 'group-spent',
    name: '分组花费',
    desc: '当前分组累计预估花费（仅单启用平台分组）',
    defaultOptions: const {'prefix': r'$'},
    toPreview: (o) => '${_nullish(o, 'prefix', r'$')}1.23',
    fields: const [
      SegmentField(
        key: 'prefix',
        label: '前缀',
        type: 'string',
        placeholder: r'$',
      ),
    ],
  ),
  SegmentDef(
    type: 'group-window-cost',
    name: '周期折算',
    desc: '当前分组本配额周期折算花费 \$（coding plan 平台自窗口起点累计 est_cost，0/非 coding 隐藏）',
    defaultOptions: const {'prefix': '折算\$'},
    toPreview: (o) => '${_nullish(o, 'prefix', '折算\$')}4.56',
    fields: const [
      SegmentField(
        key: 'prefix',
        label: '前缀',
        type: 'string',
        placeholder: '折算\$',
      ),
    ],
  ),
  SegmentDef(
    type: 'group-route',
    name: '分组·平台',
    desc: '当前分组名 / 最近一次成功命中的平台名（多平台组也可用）',
    toPreview: (o) => 'glm/GLM-自用',
  ),
  SegmentDef(
    type: 'group-coding',
    name: 'Coding Plan',
    desc: 'Coding Plan 各档利用率（动态色：fast红 / normal黄 / busy绿，红时显重置）',
    defaultOptions: const {'dynamicColor': false},
    toPreview: (o) => '5h 23%·7d 41%',
    fields: const [
      SegmentField(
        key: 'dynamicColor',
        label: '动态色 (按 pace)',
        type: 'select',
        options: ['false', 'true'],
      ),
    ],
  ),
  SegmentDef(
    type: 'group-requests',
    name: '请求·成功率',
    desc: '当前分组请求数 · 成功率（仅单启用平台分组）',
    toPreview: (o) => '128·99%',
  ),
  SegmentDef(
    type: 'group-cache',
    name: '缓存率',
    desc: '当前分组缓存命中率（仅单启用平台分组）',
    defaultOptions: const {'prefix': '缓存 '},
    toPreview: (o) => '${_nullish(o, 'prefix', '缓存 ')}37%',
    fields: const [
      SegmentField(
        key: 'prefix',
        label: '前缀',
        type: 'string',
        placeholder: '缓存 ',
      ),
    ],
  ),
  SegmentDef(
    type: 'group-tokens',
    name: '总 Tokens',
    desc: '当前分组已使用总 tokens（仅单启用平台分组）',
    defaultOptions: const {'prefix': ''},
    toPreview: (o) => '${_nullish(o, 'prefix', '')}1.2M',
    fields: const [
      SegmentField(key: 'prefix', label: '前缀', type: 'string', placeholder: ''),
    ],
  ),
  // ── 原子段：一个原始 statusline 输入字段一个 ──
  SegmentDef(
    type: 'cost-usd',
    name: '成本 (\$)',
    desc: 'cost.total_cost_usd — 累计预估成本',
    defaultOptions: const {'prefix': r'$'},
    toPreview: (o) => '${_nullish(o, 'prefix', r'$')}0.12',
    fields: const [
      SegmentField(
        key: 'prefix',
        label: '前缀',
        type: 'string',
        placeholder: r'$',
      ),
    ],
  ),
  SegmentDef(
    type: 'session-duration',
    name: '会话耗时',
    desc: 'cost.total_duration_ms — 会话总耗时',
    defaultOptions: const {'format': 'human'},
    toPreview: (o) => o['format'] == 'ms' ? '285000ms' : '4m45s',
    fields: const [
      SegmentField(
        key: 'format',
        label: '格式',
        type: 'select',
        options: ['human', 'ms'],
      ),
    ],
  ),
  SegmentDef(
    type: 'api-duration',
    name: 'API 耗时',
    desc: 'cost.total_api_duration_ms — API 等待时间',
    defaultOptions: const {'format': 'human'},
    toPreview: (o) => o['format'] == 'ms' ? '15300ms' : '15s',
    fields: const [
      SegmentField(
        key: 'format',
        label: '格式',
        type: 'select',
        options: ['human', 'ms'],
      ),
    ],
  ),
  SegmentDef(
    type: 'lines-changed',
    name: '代码变更',
    desc: 'cost.total_lines_added / removed — 新增/删除行',
    toPreview: (o) => '+412 -87',
  ),
  SegmentDef(
    type: 'context-tokens',
    name: '上下文 Tokens',
    desc: '输入/输出 token，或 session 合计（total_input + total_output）',
    defaultOptions: const {'abbrev': true, 'mode': 'split'},
    toPreview: (o) => o['mode'] == 'sum'
        ? (jsTruthy(o['abbrev']) ? '101.9K' : '101900')
        : (jsTruthy(o['abbrev']) ? '89.5K/12.4K' : '89500/12400'),
    fields: const [
      SegmentField(
        key: 'mode',
        label: '模式',
        type: 'select',
        options: ['split', 'sum'],
      ),
      SegmentField(
        key: 'abbrev',
        label: '缩写 (K/M)',
        type: 'select',
        options: ['true', 'false'],
      ),
    ],
  ),
  SegmentDef(
    type: 'context-max',
    name: '上下文容量',
    desc: 'context_window.context_window_size — 最大窗口',
    defaultOptions: const {'abbrev': true},
    toPreview: (o) => jsTruthy(o['abbrev']) ? '200K' : '200000',
    fields: const [
      SegmentField(
        key: 'abbrev',
        label: '缩写 (K/M)',
        type: 'select',
        options: ['true', 'false'],
      ),
    ],
  ),
  SegmentDef(
    type: 'context-remaining',
    name: '上下文剩余',
    desc: 'context_window.remaining_percentage — 剩余百分比',
    toPreview: (o) => '49%',
  ),
  SegmentDef(
    type: 'context-cache',
    name: '缓存率',
    desc: '缓存写入/读取 token，或缓存命中率 %（≤4 位小数）',
    defaultOptions: const {'abbrev': true, 'mode': 'tokens', 'prefix': '缓存 '},
    toPreview: (o) => o['mode'] == 'hitrate'
        ? '${_nullish(o, 'prefix', '缓存 ')}13.3578%'
        : (jsTruthy(o['abbrev']) ? 'w20K r12.1K' : 'w20000 r12100'),
    fields: const [
      SegmentField(
        key: 'mode',
        label: '模式',
        type: 'select',
        options: ['tokens', 'hitrate'],
      ),
      SegmentField(
        key: 'abbrev',
        label: '缩写 (K/M)',
        type: 'select',
        options: ['true', 'false'],
      ),
      SegmentField(
        key: 'prefix',
        label: '命中率前缀',
        type: 'string',
        placeholder: '缓存 ',
      ),
    ],
  ),
  SegmentDef(
    type: 'rate-limit-5h',
    name: '限制 5h',
    desc: 'rate_limits.five_hour — 5 小时窗口使用率',
    defaultOptions: const {'showReset': false},
    toPreview: (o) => jsTruthy(o['showReset']) ? '5h:34% (128m)' : '5h:34%',
    fields: const [
      SegmentField(
        key: 'showReset',
        label: '显示剩余重置时间',
        type: 'select',
        options: ['false', 'true'],
      ),
    ],
  ),
  SegmentDef(
    type: 'rate-limit-7d',
    name: '限制 7d',
    desc: 'rate_limits.seven_day — 7 天窗口使用率',
    defaultOptions: const {'showReset': false},
    toPreview: (o) => jsTruthy(o['showReset']) ? '7d:62% (40h)' : '7d:62%',
    fields: const [
      SegmentField(
        key: 'showReset',
        label: '显示剩余重置时间',
        type: 'select',
        options: ['false', 'true'],
      ),
    ],
  ),
  SegmentDef(
    type: 'git-branch',
    name: 'Git 分支',
    desc: '脚本内 git branch --show-current（非 git / 无分支降级空）',
    toPreview: (o) => 'main',
  ),
  SegmentDef(
    type: 'git-host',
    name: 'Git 主机',
    desc: 'workspace.repo.host — Git 仓库主机',
    toPreview: (o) => 'github.com',
  ),
  SegmentDef(
    type: 'git-owner',
    name: 'Git 所有者',
    desc: 'workspace.repo.owner — 仓库所有者',
    toPreview: (o) => 'anthropics',
  ),
  SegmentDef(
    type: 'git-repo',
    name: 'Git 仓库',
    desc: 'workspace.repo.name — 仓库名',
    toPreview: (o) => 'claude-code',
  ),
  SegmentDef(
    type: 'git-repo-full',
    name: 'Git 全名',
    desc: 'owner/name — 仓库完整标识',
    toPreview: (o) => 'anthropics/claude-code',
  ),
  SegmentDef(
    type: 'git-worktree',
    name: 'Git Worktree',
    desc: 'workspace.git_worktree — Git worktree 名称',
    toPreview: (o) => 'feature-xyz',
  ),
  SegmentDef(
    type: 'cwd',
    name: '工作目录',
    desc: 'workspace.current_dir — 当前工作目录',
    defaultOptions: const {'format': 'basename'},
    toPreview: (o) =>
        o['format'] == 'full' ? '/Users/luoxin/persons/aidog' : 'aidog',
    fields: const [
      SegmentField(
        key: 'format',
        label: '格式',
        type: 'select',
        options: ['basename', 'full'],
      ),
    ],
  ),
  SegmentDef(
    type: 'project-dir',
    name: '项目目录',
    desc: 'workspace.project_dir — 项目启动目录',
    defaultOptions: const {'format': 'basename'},
    toPreview: (o) =>
        o['format'] == 'full' ? '/Users/luoxin/persons/aidog' : 'aidog',
    fields: const [
      SegmentField(
        key: 'format',
        label: '格式',
        type: 'select',
        options: ['basename', 'full'],
      ),
    ],
  ),
  SegmentDef(
    type: 'added-dirs',
    name: '附加目录',
    desc: 'workspace.added_dirs — /add-dir 添加的目录',
    toPreview: (o) => 'shared,web',
  ),
  SegmentDef(
    type: 'session-id',
    name: '会话 ID',
    desc: 'session_id — 会话标识符',
    defaultOptions: const {'truncate': true},
    toPreview: (o) => jsTruthy(o['truncate']) ? 'abc123xy' : 'abc123xyz789',
    fields: const [
      SegmentField(
        key: 'truncate',
        label: '截断 (前8位)',
        type: 'select',
        options: ['true', 'false'],
      ),
    ],
  ),
  SegmentDef(
    type: 'session-name',
    name: '会话名称',
    desc: 'session_name — 自定义会话名（未设置时隐藏）',
    toPreview: (o) => 'statusline-atoms',
  ),
  SegmentDef(
    type: 'transcript-path',
    name: '记录路径',
    desc: 'transcript_path — 会话记录文件',
    defaultOptions: const {'format': 'basename'},
    toPreview: (o) => o['format'] == 'full'
        ? '/Users/luoxin/.claude/session.jsonl'
        : 'session.jsonl',
    fields: const [
      SegmentField(
        key: 'format',
        label: '格式',
        type: 'select',
        options: ['basename', 'full'],
      ),
    ],
  ),
  SegmentDef(
    type: 'worktree-name',
    name: 'Worktree 名',
    desc: 'worktree.name — Worktree 标识',
    toPreview: (o) => 'feature-xyz',
  ),
  SegmentDef(
    type: 'worktree-branch',
    name: 'Worktree 分支',
    desc: 'worktree.branch — 当前工作分支',
    toPreview: (o) => 'feat/atoms',
  ),
  SegmentDef(
    type: 'worktree-original-branch',
    name: 'Worktree 源分支',
    desc: 'worktree.original_branch — 回源分支',
    toPreview: (o) => 'main',
  ),
  SegmentDef(
    type: 'pr-number',
    name: 'PR 编号',
    desc: 'pr.number — 开放 PR 编号',
    defaultOptions: const {'prefix': '#'},
    toPreview: (o) => '${_nullish(o, 'prefix', '#')}123',
    fields: const [
      SegmentField(
        key: 'prefix',
        label: '前缀',
        type: 'string',
        placeholder: '#',
      ),
    ],
  ),
  SegmentDef(
    type: 'pr-url',
    name: 'PR 链接',
    desc: 'pr.url — PR 链接',
    toPreview: (o) => 'https://github.com/o/r/pull/123',
  ),
  SegmentDef(
    type: 'pr-state',
    name: 'PR 状态',
    desc: 'pr.review_state — PR 审查状态',
    toPreview: (o) => 'approved',
  ),
  SegmentDef(
    type: 'version',
    name: 'CC 版本',
    desc: 'version — Claude Code 版本',
    defaultOptions: const {'prefix': 'v'},
    toPreview: (o) => '${_nullish(o, 'prefix', 'v')}2.1.90',
    fields: const [
      SegmentField(
        key: 'prefix',
        label: '前缀',
        type: 'string',
        placeholder: 'v',
      ),
    ],
  ),
  SegmentDef(
    type: 'output-style',
    name: '输出风格',
    desc: 'output_style.name — 当前输出风格',
    toPreview: (o) => 'default',
  ),
  SegmentDef(
    type: 'thinking',
    name: '思考模式',
    desc: 'thinking.enabled — 扩展思考开启时显示',
    defaultOptions: const {'label': 'thinking'},
    toPreview: (o) => _nullish(o, 'label', 'thinking'),
    fields: const [
      SegmentField(
        key: 'label',
        label: '文案',
        type: 'string',
        placeholder: 'thinking',
      ),
    ],
  ),
  SegmentDef(
    type: 'token-warn',
    name: 'Token 警示',
    desc: 'exceeds_200k_tokens — 超 200k 时警示',
    defaultOptions: const {'label': '⚠200k'},
    toPreview: (o) => _nullish(o, 'label', '⚠200k'),
    fields: const [
      SegmentField(
        key: 'label',
        label: '文案',
        type: 'string',
        placeholder: '⚠200k',
      ),
    ],
  ),
  SegmentDef(
    type: 'agent',
    name: 'Agent 名称',
    desc: 'agent.name — agent 名称（未配置时隐藏）',
    toPreview: (o) => 'reviewer',
  ),
  SegmentDef(
    type: 'agent-badge',
    name: '子代理徽章',
    desc: '[type·状态·模型] — 子代理任务徽章（type 空时隐藏，状态符号/色动态）',
    toPreview: (o) => '[Agent·●·Opus]',
  ),
  SegmentDef(
    type: 'custom',
    name: '自定义',
    desc: '自定义 jq 表达式',
    defaultOptions: const {'expr': '.model.display_name'},
    toPreview: (o) => '<${_falsy(o, 'expr', '.model.display_name')}>',
    fields: const [
      SegmentField(
        key: 'expr',
        label: 'jq 表达式',
        type: 'string',
        placeholder: '.model.display_name',
      ),
    ],
  ),
];

final Map<String, SegmentDef> kSegmentDefMap = {
  for (final d in kSegmentDefs) d.type: d,
};

/// 「添加段」选择器的分类，顺序照抄 React。
class SegmentCategory {
  const SegmentCategory(this.id, this.label, this.types);
  final String id;
  final String label;
  final List<String> types;
}

const kSegmentCategories = <SegmentCategory>[
  SegmentCategory('common', '常用', [
    'model',
    'context-bar',
    'context-pct',
    'git',
    'cost',
    'rate-limits',
    'effort',
    'vim',
    'separator',
  ]),
  SegmentCategory('cost', '成本 / 执行', [
    'cost-usd',
    'session-duration',
    'api-duration',
    'lines-changed',
  ]),
  SegmentCategory('context', '上下文', [
    'context-tokens',
    'context-max',
    'context-remaining',
    'context-cache',
  ]),
  SegmentCategory('rate', '速率限制', ['rate-limit-5h', 'rate-limit-7d']),
  SegmentCategory('git', 'Git', [
    'git-branch',
    'git-host',
    'git-owner',
    'git-repo',
    'git-repo-full',
    'git-worktree',
  ]),
  SegmentCategory('session', '目录 / 会话', [
    'cwd',
    'project-dir',
    'added-dirs',
    'session-id',
    'session-name',
    'transcript-path',
  ]),
  SegmentCategory('worktree', 'Worktree', [
    'worktree-name',
    'worktree-branch',
    'worktree-original-branch',
  ]),
  SegmentCategory('pr', 'Pull Request', ['pr-number', 'pr-url', 'pr-state']),
  SegmentCategory('other', '其他', [
    'version',
    'output-style',
    'thinking',
    'token-warn',
    'agent',
    'agent-badge',
    'custom',
  ]),
];

/// 内置默认 3 行布局（PRD）。只在首次运行或显式「恢复默认布局」时应用。
const kDefaultSegments = <StatusLineSegment>[
  StatusLineSegment(
    id: 'd-model',
    type: 'model',
    enabled: true,
    newline: false,
    color: '#4A9EFF',
    options: {'format': 'short'},
  ),
  StatusLineSegment(
    id: 'd-sep1',
    type: 'separator',
    enabled: true,
    newline: false,
    options: {'char': ' · '},
  ),
  StatusLineSegment(
    id: 'd-tokens',
    type: 'context-tokens',
    enabled: true,
    newline: false,
    color: '#BF5AF2',
    options: {'mode': 'sum', 'abbrev': true},
  ),
  StatusLineSegment(
    id: 'd-cost',
    type: 'cost-usd',
    enabled: true,
    newline: false,
    color: '#8E8E93',
    options: {'prefix': r'$', 'affixPre': '[', 'affixSuf': ']·'},
  ),
  StatusLineSegment(
    id: 'd-ctx',
    type: 'context-pct',
    enabled: true,
    newline: false,
    color: '#34C759',
    options: {},
  ),
  StatusLineSegment(
    id: 'd-cache',
    type: 'context-cache',
    enabled: true,
    newline: false,
    color: '#34C759',
    options: {'mode': 'hitrate', 'prefix': '缓存 ', 'affixPre': '·'},
  ),
  StatusLineSegment(
    id: 'd-branch',
    type: 'git-branch',
    enabled: true,
    newline: true,
    color: '#FFD60A',
    options: {},
  ),
  StatusLineSegment(
    id: 'd-worktree',
    type: 'worktree-name',
    enabled: true,
    newline: false,
    options: {'affixPre': '·'},
  ),
  StatusLineSegment(
    id: 'd-cwd',
    type: 'cwd',
    enabled: true,
    newline: false,
    options: {'format': 'full', 'affixPre': '|'},
  ),
  StatusLineSegment(
    id: 'd-coding',
    type: 'group-coding',
    enabled: true,
    newline: true,
    options: {'dynamicColor': true},
  ),
  StatusLineSegment(
    id: 'd-balance',
    type: 'group-balance',
    enabled: true,
    newline: false,
    options: {'dynamicColor': true, 'prefix': r'$', 'affixPre': '·'},
  ),
  StatusLineSegment(
    id: 'd-wcost',
    type: 'group-window-cost',
    enabled: true,
    newline: false,
    options: {'prefix': r'$', 'affixPre': '·'},
  ),
  StatusLineSegment(
    id: 'd-route',
    type: 'group-route',
    enabled: true,
    newline: false,
    options: {'affixPre': '·'},
  ),
  StatusLineSegment(
    id: 'd-version',
    type: 'version',
    enabled: true,
    newline: false,
    color: '#8E8E93',
    options: {'prefix': 'v', 'affixPre': ' · '},
  ),
];

/// 内置默认 SubagentStatusLine 布局（单行）。
const kDefaultSubagentSegments = <StatusLineSegment>[
  StatusLineSegment(
    id: 'sa-badge',
    type: 'agent-badge',
    enabled: true,
    newline: false,
    options: {},
  ),
  StatusLineSegment(
    id: 'sa-name',
    type: 'custom',
    enabled: true,
    newline: false,
    color: '#4A9EFF',
    options: {'expr': '.label // .name // .id // "?"'},
  ),
  StatusLineSegment(
    id: 'sa-ctx',
    type: 'context-pct',
    enabled: true,
    newline: false,
    color: '#34C759',
    options: {'suffix': '%', 'degradeZero': true, 'affixPre': '·'},
  ),
  StatusLineSegment(
    id: 'sa-tokens',
    type: 'context-tokens',
    enabled: true,
    newline: false,
    color: '#BF5AF2',
    options: {'mode': 'sum', 'abbrev': true, 'affixPre': '·'},
  ),
  StatusLineSegment(
    id: 'sa-dur',
    type: 'session-duration',
    enabled: true,
    newline: false,
    color: '#8E8E93',
    options: {'format': 'human', 'affixPre': '·'},
  ),
];

/// 可用数据字段参考表。
class DataFieldGroup {
  const DataFieldGroup(this.id, this.group, this.fields);
  final String id;
  final String group;
  final List<(String key, String desc)> fields;
}

const kStatuslineDataFields = <DataFieldGroup>[
  DataFieldGroup('model', '模型', [
    ('model.id', '模型标识符'),
    ('model.display_name', '模型显示名称'),
  ]),
  DataFieldGroup('workspace', '工作区', [
    ('workspace.current_dir', '当前工作目录'),
    ('workspace.project_dir', '项目启动目录'),
    ('workspace.repo.owner/name', 'Git 仓库标识'),
  ]),
  DataFieldGroup('cost', '成本', [
    ('cost.total_cost_usd', '累计预估成本 (\$)'),
    ('cost.total_duration_ms', '总持续时间 (ms)'),
    ('cost.total_api_duration_ms', 'API 等待时间 (ms)'),
  ]),
  DataFieldGroup('contextWindow', '上下文窗口', [
    ('context_window.used_percentage', '已使用百分比'),
    ('context_window.context_window_size', '最大窗口大小'),
  ]),
  DataFieldGroup('rateLimits', '速率限制', [
    ('rate_limits.five_hour.used_percentage', '5小时窗口使用 %'),
    ('rate_limits.seven_day.used_percentage', '7天窗口使用 %'),
  ]),
  DataFieldGroup('other', '其他', [
    ('effort.level', '推理工作量'),
    ('vim.mode', 'Vim 模式'),
    ('session_id', '会话 ID'),
    ('version', 'Claude Code 版本'),
  ]),
  DataFieldGroup('subagent', '子代理任务', [
    ('type', '任务类型（如 local_agent；空则隐藏徽章）'),
    ('status', '任务状态（running/pending/completed/failed/cancelled）'),
    ('agent.name', '子代理名称'),
  ]),
];

// ── 纯函数 ────────────────────────────────────────────────

/// 一行：对齐方式 + 段。
class SegmentRow {
  const SegmentRow(this.align, this.segs);
  final RowAlign align;
  final List<StatusLineSegment> segs;
}

/// 按 `newline` 把段切成行。
List<SegmentRow> groupRows(List<StatusLineSegment> segments) {
  final rows = <SegmentRow>[];
  List<StatusLineSegment>? cur;
  for (final seg in segments) {
    if (cur == null || (seg.newline && cur.isNotEmpty)) {
      cur = <StatusLineSegment>[];
      rows.add(SegmentRow(seg.align ?? RowAlign.left, cur));
    }
    cur.add(seg);
  }
  return rows;
}

/// 重新推导 `newline`：第一段永远不带显式换行（它的行首是隐式的）。
List<StatusLineSegment> normalizeSegments(List<StatusLineSegment> segments) {
  if (segments.isEmpty) return segments;
  return [
    for (var i = 0; i < segments.length; i++)
      if (i == 0 && segments[i].newline)
        segments[i].copyWith(newline: false)
      else
        segments[i],
  ];
}

/// 该段是否是行首（启用段里的第一个，或带 `newline`）。
bool isRowLeaderSeg(List<StatusLineSegment> segments, String id) {
  final active = segments.where((s) => s.enabled).toList();
  final idx = active.indexWhere((s) => s.id == id);
  if (idx < 0) {
    // 停用的段：只有显式 newline 才算行首。
    for (final s in segments) {
      if (s.id == id) return s.newline;
    }
    return false;
  }
  return idx == 0 || active[idx].newline;
}

/// 驱动 autoColor 预览的模拟指标（对齐 bash 侧阈值）。
const kPreviewMetric = <String, num>{
  'context-pct': 65,
  'context-bar': 65,
  'cost': 12,
  'cost-usd': 12,
  'rate-limits': 41,
  'rate-limit-5h': 34,
  'rate-limit-7d': 62,
  'context-remaining': 49,
  'session-duration': 285,
  'api-duration': 15,
};

/// 模拟指标 → 语义分级（与 bash 阈值同源）。
ColorLevel autoColorPreviewLevel(String type) {
  final m = kPreviewMetric[type] ?? 0;
  if (type == 'cost' || type == 'cost-usd') {
    if (m > 1000) return ColorLevel.danger;
    if (m > 100) return ColorLevel.warning;
    return ColorLevel.success;
  }
  if (type == 'context-remaining') {
    if (m < 20) return ColorLevel.danger;
    if (m < 40) return ColorLevel.warning;
    return ColorLevel.success;
  }
  if (type == 'session-duration' || type == 'api-duration') {
    if (m > 300) return ColorLevel.danger;
    if (m > 60) return ColorLevel.warning;
    return ColorLevel.success;
  }
  if (m > 80) return ColorLevel.danger;
  if (m > 60) return ColorLevel.warning;
  return ColorLevel.success;
}

/// 段的预览色：autoColor 走分级，否则走固定 hex；都没有返回 null。
Color? previewColor(StatusLineSegment seg, AidogColors c) {
  if (seg.autoColor == true && kValueColorable.contains(seg.type)) {
    return levelColor(autoColorPreviewLevel(seg.type), c);
  }
  final rgb = hexToRgb(seg.color);
  return rgb == null ? null : Color.fromARGB(255, rgb[0], rgb[1], rgb[2]);
}

/// 段的有效选项 = 定义的默认 + 用户覆盖。
Map<String, Object?> effectiveOptions(SegmentDef def, StatusLineSegment seg) =>
    {...def.defaultOptions, ...seg.options};

// ── 段列表的结构性变更（useStatusLinePanel 的纯计算部分） ──

/// 按行首 id 删掉**整行**。
///
/// 行归属从**当前全量段**（含停用段，与渲染分组同一套）推导，所以视觉上的行
/// 与删掉的集合永远一致 —— 这修的是「把段拖到别行后，删该行会删错段」那个 bug。
List<StatusLineSegment> deleteRow(
  List<StatusLineSegment> segments,
  String leaderId,
) {
  final rows = <List<StatusLineSegment>>[];
  List<StatusLineSegment>? cur;
  for (final seg in segments) {
    if (cur == null || (seg.newline && cur.isNotEmpty)) {
      cur = <StatusLineSegment>[];
      rows.add(cur);
    }
    cur.add(seg);
  }
  final row = rows.where((r) => r.any((s) => s.id == leaderId)).firstOrNull;
  if (row == null) return segments;
  final ids = row.map((s) => s.id).toSet();
  return segments.where((s) => !ids.contains(s.id)).toList();
}

/// 把某段所在行的对齐方式向后轮换一格（写在该行的行首段上）。
List<StatusLineSegment> cycleRowAlign(
  List<StatusLineSegment> segments,
  String segId,
) {
  final active = segments.where((s) => s.enabled).toList();
  final idx = active.indexWhere((s) => s.id == segId);
  if (idx < 0) return segments;
  var leaderIdx = idx;
  while (leaderIdx > 0 && !active[leaderIdx].newline) {
    leaderIdx--;
  }
  final leaderId = active[leaderIdx].id;
  const order = [RowAlign.left, RowAlign.center, RowAlign.right];
  final cur = active[leaderIdx].align ?? RowAlign.left;
  final next = order[(order.indexOf(cur) + 1) % order.length];
  return [
    for (final s in segments)
      if (s.id == leaderId) s.copyWith(align: next) else s,
  ];
}

/// 新建一个段。[id] 由调用方给（React 用 `s${Date.now()}`）。
StatusLineSegment? makeSegment(
  String type, {
  bool newline = false,
  required String id,
}) {
  final def = kSegmentDefMap[type];
  if (def == null) return null;
  return StatusLineSegment(
    id: id,
    type: type,
    enabled: true,
    newline: newline,
    options: {...def.defaultOptions},
  );
}
