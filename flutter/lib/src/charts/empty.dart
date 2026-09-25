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
    // React 的空态盒是 `minHeight: 160` 的居中列（`ChartCard.tsx:32-48`）：
    // 没有这个下限，无数据的卡会塌成两行字，整页高度跟着跳。
    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: 160),
      child: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              // 空态主文案 13（`ChartCard.tsx:44`）。
              text,
              style: AidogType.label.copyWith(fontSize: 13, color: t.c.fg2),
            ),
            if (hint != null) ...[
              const SizedBox(height: AidogSpace.sxs),
              Text(
                // 副行 11（`ChartCard.tsx:46`）。
                hint!,
                style: AidogType.caption.copyWith(fontSize: 11, color: t.c.fg3),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
