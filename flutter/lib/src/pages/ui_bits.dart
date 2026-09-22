/// 票 I07 三个页面共用的小零件。
///
/// 放这里而不是各页一份，是因为「确认卡」的形状是本票的一条硬要求：
/// 破坏性操作必须先确认，而确认态要能被 widget 测试直接断言。三页各写一个
/// 就会各自漂移，测试也得写三遍。
///
/// 色值一律 `AidogTheme.of(context).c.*`，本文件零硬编码颜色。
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

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

/// 开关滑块，对齐 React `src/styles/globals.css:507-540` 的 `.toggle`：
/// 40×22 轨道 + 16 圆点，开态轨道填 accent、圆点右移 18，250ms 过渡。
///
/// Flutter 自带的 `Switch` 长得是 Material 的样子（尺寸、水波纹、拇指阴影都对不上），
/// 要调到这个形状得覆盖七八个属性，不如直接画两个盒子。
class AidogSwitch extends StatelessWidget {
  const AidogSwitch({
    super.key,
    required this.value,
    required this.onChanged,
    this.tooltip,
  });

  final bool value;

  /// null = 只读（点不动）。
  final VoidCallback? onChanged;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final c = AidogTheme.of(context).c;
    return Tooltip(
      message: tooltip ?? '',
      child: MouseRegion(
        cursor: onChanged == null
            ? SystemMouseCursors.basic
            : SystemMouseCursors.click,
        child: GestureDetector(
          onTap: onChanged,
          child: AnimatedContainer(
            duration: AidogMotion.slow,
            curve: AidogMotion.easeStandard,
            width: 40,
            height: 22,
            decoration: BoxDecoration(
              color: value ? c.accent : c.surface2,
              border: Border.all(color: value ? c.accent : c.line),
              borderRadius: BorderRadius.circular(11),
            ),
            child: AnimatedAlign(
              duration: AidogMotion.slow,
              curve: AidogMotion.easeStandard,
              alignment: value
                  ? AlignmentDirectional.centerEnd
                  : AlignmentDirectional.centerStart,
              child: Container(
                margin: const EdgeInsets.symmetric(horizontal: 2),
                width: 16,
                height: 16,
                // React 写死 `#fff`，深浅两套一样。取浅色模式的 surface 当「白」，
                // 与 [AidogModal] 拿 `AidogColors.dark.bg` 当「黑」同一条路子：不写字面色值。
                decoration: BoxDecoration(
                  color: AidogColors.light.surface,
                  shape: BoxShape.circle,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// 真浮层弹窗：把 [child] 画进根 Overlay，覆盖整窗、居中、带遮罩。
///
/// 用 `OverlayPortal` 而不是 `showDialog`（用户 2026-09-22 拍板要浮层，票 11）：
/// 全应用 20 多处弹窗的开合都是页面 state 的一个字段（`if (x != null) XxxCard(...)`），
/// `showDialog` 要把它们全改成命令式的 `await`，还得各自处理 `mounted`；
/// `OverlayPortal` 保留声明式写法，同时**元素树祖先不变** —— `AidogTheme.of` /
/// `AidogI18n.of` / `Material` 全部照常向上找得到，`find.byType` 也照常命中。
/// 对齐的是 React 那边 Radix 的 Portal：浮在页面之上、按窗口居中、点遮罩的行为
/// 由调用方按 React 的 `Dialog`（可关）/ `AlertDialog`（不可关）逐个决定。
class AidogModal extends StatefulWidget {
  const AidogModal({
    super.key,
    required this.child,
    this.onBarrierTap,
    this.onEscape,
    this.maxWidth = 420,
  });

  final Widget child;

  /// null = 点遮罩不关闭，对齐 React 的 `AlertDialog`（破坏性确认不许误触关掉）。
  final VoidCallback? onBarrierTap;

  /// 按 Esc 关闭。**不传就跟随 [onBarrierTap]** —— 十八处调用点里有十七处两者
  /// 就是同一个关闭函数，默认跟随省掉十七行重复。
  ///
  /// 要单独传的只有一种情形：点遮罩不关、但按 Esc 要关。Radix 的 `AlertDialog`
  /// 正是这样（`onEscapeKeyDown` 不拦就关，外部点击一律不关），[ConfirmCard] 照此传。
  final VoidCallback? onEscape;

  /// 面板最大宽度，对齐 React 各弹窗的 `maxWidth`。
  final double maxWidth;

  @override
  State<AidogModal> createState() => _AidogModalState();
}

class _AidogModalState extends State<AidogModal> {
  final _controller = OverlayPortalController();

  @override
  void initState() {
    super.initState();
    // 这个 widget 只在「该显示」时才被挂进树，所以挂上即显示、拔掉即消失。
    _controller.show();
  }

  @override
  Widget build(BuildContext context) => OverlayPortal(
    controller: _controller,
    overlayChildBuilder: (context) {
      final barrier = widget.onBarrierTap;
      return _EscapeScope(
        onEscape: widget.onEscape ?? barrier,
        child: Material(
        type: MaterialType.transparency,
        child: Stack(
          children: [
            Positioned.fill(
              child: GestureDetector(
                onTap: barrier,
                // 遮罩色不进 token 表：React 那边是写死的 `bg-black/80`，
                // 深浅两套都一样。取深色模式的底色 token 当「黑」，不写字面色值。
                child: ColoredBox(
                  color: AidogColors.dark.bg.withValues(alpha: 0.72),
                ),
              ),
            ),
            Center(
              child: ConstrainedBox(
                constraints: BoxConstraints(maxWidth: widget.maxWidth),
                child: SingleChildScrollView(
                  padding: const EdgeInsets.all(AidogSpace.s_2xl),
                  child: widget.child,
                ),
              ),
            ),
          ],
        ),
        ),
      );
    },
  );
}

/// 让 Esc 关掉浮层。
///
/// 为什么不靠 Flutter 默认的 Escape→`DismissIntent`：浮层里有输入框时焦点在
/// `EditableText` 上，键盘事件先到它那儿；默认那条链在部分平台会被输入框自己消费掉。
/// 这里显式写一条 `Shortcuts`，并且把它放在 [FocusScope] **外面** —— 事件从焦点
/// 节点往祖先冒泡，输入框没处理的 Escape 一定会走到这里。
///
/// 两层浮层叠着时只关最上面一层：每层浮层各自是 Overlay 的一个孩子，互不为祖先，
/// 冒泡只会走到**当前有焦点的那一层**的 `Shortcuts`，下面那层收不到。
class _EscapeScope extends StatelessWidget {
  const _EscapeScope({required this.child, this.onEscape});

  final Widget child;
  final VoidCallback? onEscape;

  @override
  Widget build(BuildContext context) {
    final onEscape = this.onEscape;
    if (onEscape == null) return child;
    return Shortcuts(
      shortcuts: const {
        SingleActivator(LogicalKeyboardKey.escape): DismissIntent(),
      },
      child: Actions(
        actions: {
          DismissIntent: CallbackAction<DismissIntent>(
            onInvoke: (_) {
              onEscape();
              return null;
            },
          ),
        },
        child: FocusScope(autofocus: true, child: child),
      ),
    );
  }
}

/// 破坏性操作的确认弹窗。
///
/// 浮层由 [AidogModal] 提供，卡片本体沿用 [Tile] 的长相（与 React 那边
/// `AlertDialogContent` 挂 `glass-elevated` 同路）。`find.byType(ConfirmCard)`
/// 仍然命中 —— `OverlayPortal` 的浮层子树仍在同一棵元素树里。
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
    this.dismissOnBarrier = false,
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

  /// 点遮罩关不关。缺省 false = React 的 `AlertDialog` 语义（破坏性确认不许误触关掉）；
  /// React 侧用普通 `Dialog` 的那几处传 true。执行中（[busy]）一律不关，同 React
  /// 的 `onPointerDownOutside` busy 守卫。
  final bool dismissOnBarrier;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    return AidogModal(
      onBarrierTap: dismissOnBarrier && !busy ? onCancel : null,
      // 点遮罩关不关由 dismissOnBarrier 说了算，但 Esc 一律关（除非正忙）——
      // 与 Radix `AlertDialog` 同口径：外部点击不关，Escape 关。
      onEscape: busy ? null : onCancel,
      child: Tile(
        title: title,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(body, style: AidogType.micro.copyWith(color: theme.c.fg2)),
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

/// 「这个维度 pi 不支持」的说明条 —— 对齐
/// `src/components/shared/PiUnsupportedNote.tsx`。
///
/// 用在 MCP / 通知 hook / cc-switch 导入三处：那里对 Claude Code 与 Codex 有内容、
/// 对 pi 永远是空的。不写一句就像 aidog 坏了。刻意做成中性提示（不是错误态、
/// 也不是空状态），因为这是产品决定，不是缺陷 ——
/// 见 `docs/adr/0002-no-mcp-hooks-or-statusline-for-pi.md`。
///
/// [reasonKey] 由调用方给，三处各不相同，不能在这里写死。
class PiUnsupportedNote extends StatelessWidget {
  const PiUnsupportedNote({
    super.key,
    required this.reasonKey,
    required this.reasonFallback,
  });

  final String reasonKey;
  final String reasonFallback;

  @override
  Widget build(BuildContext context) {
    final t = AidogI18n.of(context);
    final theme = AidogTheme.of(context);
    final reason = t.t(reasonKey);
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AidogSpace.smd,
        vertical: AidogSpace.ssm,
      ),
      decoration: BoxDecoration(
        color: theme.c.surface2,
        border: Border.all(color: theme.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.md),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // React 那边是 pi.svg 的 <img>；这里没有这份资产，用同尺寸的占位图标。
          Icon(Icons.extension_outlined, size: 16, color: theme.c.fg2),
          const SizedBox(width: AidogSpace.ssm),
          Expanded(
            child: Text.rich(
              TextSpan(
                children: [
                  TextSpan(
                    text: t.t('pi.unsupportedTitle'),
                    style: AidogType.micro.copyWith(
                      color: theme.c.fg,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const TextSpan(text: ' — '),
                  TextSpan(text: reason == reasonKey ? reasonFallback : reason),
                ],
              ),
              style: AidogType.micro.copyWith(color: theme.c.fg2),
            ),
          ),
        ],
      ),
    );
  }
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
