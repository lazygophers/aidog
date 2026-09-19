/// 诚实空态：无数据时**不画零值假图**（对应 React 版 `ChartCard` 的 `empty` 分支）。
///
/// 卡片壳、标题、副题由 `SeriesTile` 负责（票 I02 的四种格子之一，不另造第五种），
/// 这里只填图表插槽本身。文案由调用方传（i18n 归票 I03，本层不碰）。
library;

import 'package:flutter/material.dart';

import '../shell/theme.dart';

class ChartEmpty extends StatelessWidget {
  const ChartEmpty(this.text, {super.key, this.hint});

  final String text;
  final String? hint;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(text, style: AidogType.label.copyWith(color: t.c.fg2)),
          if (hint != null) ...[
            const SizedBox(height: AidogSpace.sxs),
            Text(hint!, style: AidogType.caption.copyWith(color: t.c.fg3)),
          ],
        ],
      ),
    );
  }
}
