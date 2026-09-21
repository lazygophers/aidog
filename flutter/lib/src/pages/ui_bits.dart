/// 票 I07 三个页面共用的小零件。
///
/// 放这里而不是各页一份，是因为「确认卡」的形状是本票的一条硬要求：
/// 破坏性操作必须先确认，而确认态要能被 widget 测试直接断言。三页各写一个
/// 就会各自漂移，测试也得写三遍。
///
/// 色值一律 `AidogTheme.of(context).c.*`，本文件零硬编码颜色。
library;

import 'package:flutter/material.dart';

import '../../i18n.dart';
import '../shell/theme.dart';
import '../shell/tiles.dart';

/// 页面里的小按钮。`onTap` 为 null = 禁用态（颜色变浅且点不动）——
/// 「能不能点」就是 React 那边 `disabled=` 的直接投影，测试靠它断言校验。
class SmallButton extends StatelessWidget {
  const SmallButton({
    super.key,
    required this.label,
    this.onTap,
    this.danger = false,
    this.active = false,
  });

  final String label;
  final VoidCallback? onTap;

  /// 破坏性动作（删除 / 清空）：用 bad 色，让它和旁边的按钮长得不一样。
  final bool danger;
  final bool active;

  bool get enabled => onTap != null;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    final fg = onTap == null
        ? theme.c.fg3
        : danger
        ? theme.c.bad
        : active
        ? theme.c.accentText
        : theme.c.fg2;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(AidogRadius.sm),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        decoration: BoxDecoration(
          color: active ? theme.c.accentWash : null,
          border: Border.all(color: theme.c.line),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: Text(label, style: AidogType.micro.copyWith(color: fg)),
      ),
    );
  }
}

/// 破坏性操作的确认卡。
///
/// **故意不是 `showDialog`**：确认态是页面 state 的一部分，widget 测试里
/// `find.byType(ConfirmCard)` 就能断言它在不在，不必去 dialog 的 route 里捞。
/// （项目 CLAUDE.md 里那条「弹窗必须 createPortal」是 CSS 的坑，只对 Web 侧成立。）
class ConfirmCard extends StatelessWidget {
  const ConfirmCard({
    super.key,
    required this.title,
    required this.body,
    required this.confirmLabel,
    required this.onCancel,
    required this.onConfirm,
    this.busy = false,
    this.extra,
  });

  final String title;
  final String body;
  final String confirmLabel;
  final VoidCallback onCancel;

  /// null = 确认按钮禁用（候选清单为空之类，点了也没意义）。
  final VoidCallback? onConfirm;

  /// 执行中：两个按钮都禁掉，防止重复提交。
  final bool busy;

  /// 标题与按钮之间的额外内容（跨组警告清单、单选项之类）。
  final Widget? extra;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      child: Tile(
        title: title,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              body,
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
            if (extra != null) ...[
              const SizedBox(height: AidogSpace.ssm),
              extra!,
            ],
            const SizedBox(height: AidogSpace.ssm),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                SmallButton(
                  label: t.t('action.cancel'),
                  onTap: busy ? null : onCancel,
                ),
                const SizedBox(width: AidogSpace.ssm),
                SmallButton(
                  label: confirmLabel,
                  danger: true,
                  onTap: busy ? null : onConfirm,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// 空态 / 加载态的一句话。**不画零值假图**（与 I04 的 `ChartEmpty` 同一条规矩）。
class CenteredNote extends StatelessWidget {
  const CenteredNote({super.key, required this.text});

  final String text;

  @override
  Widget build(BuildContext context) => Tile(
    child: Padding(
      padding: const EdgeInsets.symmetric(vertical: AidogSpace.s_2xl),
      child: Center(
        child: Text(
          text,
          style: AidogType.micro.copyWith(color: AidogTheme.of(context).c.fg3),
        ),
      ),
    ),
  );
}

/// 操作结果提示条。React 那边是浮层 toast，这里挂在页面底部 —— 同样 3 秒后消失，
/// 由页面的计时器负责清掉。
class ToastBar extends StatelessWidget {
  const ToastBar({super.key, required this.text, required this.ok});

  final String text;
  final bool ok;

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: AidogSpace.ssm),
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.symmetric(
          horizontal: AidogSpace.smd,
          vertical: AidogSpace.ssm,
        ),
        decoration: BoxDecoration(
          color: theme.c.surface2,
          border: Border.all(color: ok ? theme.c.ok : theme.c.bad),
          borderRadius: BorderRadius.circular(AidogRadius.sm),
        ),
        child: Text(
          text,
          style: AidogType.micro.copyWith(color: ok ? theme.c.ok : theme.c.bad),
        ),
      ),
    );
  }
}
