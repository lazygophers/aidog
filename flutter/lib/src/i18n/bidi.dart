/// 阿拉伯语下 Flutter 的 [Directionality] 会把整页镜像，绝大多数控件跟着翻就对了。
/// 这个文件收的是**不该跟着翻**或**翻错了**的四类，都是搭原型时撞出来的，不是上线后才发现的。
///
/// | # | 症状 | 处置 |
/// |---|---|---|
/// | 1 | 图表时间轴：文字翻了柱子没翻，轴与数据对不上 | [AlwaysLtr] 包住绘图区，时间轴永远左→右（用户定） |
/// | 2 | 24 段高峰时钟带刻度翻成「23…00」，读起来是倒的 | [kHourTicks] 锁死 0→23，外面再套 [AlwaysLtr] |
/// | 3 | `⌘C` 被镜像成 `C⌘` | [ltr] 整串标 ltr |
/// | 4 | `24 小时趋势` → `小时趋势 24`、`+6.1%` → `6.1%+` | [ltr] 整串包起来（只设 textDirection 不管用，见下） |
library;

import 'package:flutter/widgets.dart';

/// Unicode 双向算法的「隔离」符号（UAX #9）。
/// <https://www.unicode.org/reports/tr9/#Explicit_Directional_Isolates>
const String _lri = '\u2066'; // LEFT-TO-RIGHT ISOLATE
const String _pdi = '\u2069'; // POP DIRECTIONAL ISOLATE

/// 漏点 3 + 4：把整串锁成从左往右，并与周围文字隔离。
///
/// 为什么不是 `Text(s, textDirection: TextDirection.ltr)`：`textDirection` 定的是
/// **段落基准方向**，而 bidi 重排发生在段落内部。一串以数字开头或结尾的短标签
/// （`24 小时趋势`、`+6.1%`），数字是弱方向字符（EN/ET），在 RTL 段落里会被算法
/// 甩到另一头 —— 段落方向设对了也照样重排。LRI…PDI 是把这一段单独拎成一个
/// 方向隔离区，里面按 LTR 解析，外面当成一个中性对象，两边都不再互相影响。
///
/// 用在哪：快捷键（`⌘C`）、带数字的短标签、百分比、版本号、纯 ASCII 的标识符。
/// **不要**用在整句正文上 —— 那会把阿拉伯语句子的词序也按 LTR 摆。
String ltr(String text) => text.isEmpty ? text : '$_lri$text$_pdi';

/// 去掉 [ltr] 加的隔离符号。给测试与「拿去复制到剪贴板」这类场景用 ——
/// 隔离符是零宽不可见字符，粘出去会变成脏数据。
String stripIsolates(String text) =>
    text.replaceAll(_lri, '').replaceAll(_pdi, '');

/// 漏点 1 + 2：包住的子树永远按从左往右排，跟界面语言无关。
///
/// 时间轴是一条数轴，不是文字：柱子 / 折线 / 热力格的位置由数据算出来，
/// 在 RTL 下并不会跟着镜像，只有轴上的文字会翻 —— 结果就是轴和数据对不上。
/// 与其让两边都翻（改绘图逻辑，且「时间往左走」本身也难读），不如让轴不翻。
/// 时间轴永远左→右是用户拍的板。
class AlwaysLtr extends StatelessWidget {
  const AlwaysLtr({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) =>
      Directionality(textDirection: TextDirection.ltr, child: child);
}

/// 漏点 2：24 段高峰时钟带 / 小时热力图的刻度顺序，锁死 0 → 23。
///
/// 单独拎出来是因为它和漏点 1 的成因不同：轴翻不翻是布局问题，刻度顺序是**数据顺序**
/// 问题。谁要是哪天在 RTL 下 `reversed` 一下「让它顺眼」，刻度就成了「23…00」。
const List<int> kHourTicks = <int>[
  0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, //
  12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
];

/// 小时刻度文字，两位补零并按 [ltr] 隔离（`08` 这种也是数字开头的短串，同样会被重排）。
String hourTickLabel(int hour) => ltr(hour.toString().padLeft(2, '0'));
