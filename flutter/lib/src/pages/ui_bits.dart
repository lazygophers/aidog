/// 票 I07 三个页面共用的小零件。
///
/// 放这里而不是各页一份，是因为「确认卡」的形状是本票的一条硬要求：
/// 破坏性操作必须先确认，而确认态要能被 widget 测试直接断言。三页各写一个
/// 就会各自漂移，测试也得写三遍。
///
/// 色值一律 `AidogTheme.of(context).c.*`，本文件零硬编码颜色。
library;

import 'dart:async';

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
    this.ghost = false,
    this.pill = false,
    this.activeTone,
    this.tooltip,
  });

  final String label;
  final VoidCallback? onTap;

  /// 破坏性动作（删除 / 清空）：用 bad 色，让它和旁边的按钮长得不一样。
  final bool danger;
  final bool active;

  /// 弱化样式：去掉描边、文字用 fg3。对齐 React 的 `<Button variant="ghost">`，
  /// 用在「排在主动作旁边、但不该抢视线」的次要动作上（如清理失效平台）。
  final bool ghost;

  /// 胶囊形（圆角 999）+ 选中时描边也换成 accent。用在「一排里挑几个」的多选
  /// 标签上（分组归属），对齐 `formSections.tsx:1083-1102` 的 pill。
  final bool pill;

  /// [active] 时改用这个色（文字 / 底 / 描边）。给的是**语义色**：
  /// 端点的 Coding Plan「C」用绿，因为绿 = 走 coding 套餐，通用高亮色说不出这层意思。
  final Color? activeTone;

  /// 悬浮解释，对应 React 挂在按钮 `title=` 上的那句。
  /// 按钮**被禁用**时尤其不能省：只禁不解释，用户不知道还差什么。
  final String? tooltip;

  bool get enabled => onTap != null;

  @override
  Widget build(BuildContext context) {
    final tip = tooltip;
    if (tip != null && tip.isNotEmpty) {
      return Tooltip(message: tip, child: _button(context));
    }
    return _button(context);
  }

  Widget _button(BuildContext context) {
    final theme = AidogTheme.of(context);
    final fg = onTap == null
        ? theme.c.fg3
        : danger
        ? theme.c.bad
        : active
        ? (activeTone ?? theme.c.accentText)
        : ghost
        ? theme.c.fg3
        : theme.c.fg2;
    final radius = BorderRadius.circular(pill ? 999 : AidogRadius.sm);
    final bg = active
        ? (activeTone?.withValues(alpha: 0.08) ?? theme.c.accentWash)
        : Colors.transparent;
    final borderColor = ghost
        ? Colors.transparent
        : active && activeTone != null
        ? activeTone!.withValues(alpha: 0.25)
        : pill && active
        ? theme.c.accent
        : theme.c.line;
    // 底色与描边交给 `Material` 自己插值（它的 `animationDuration` 管 color /
    // shape），水波画在同一张 `Material` 上 —— 这样「有过渡」和「有水波」能同时成立。
    //
    // 🔴 别改回 `Container` / `AnimatedContainer` 包一层：`InkWell` 的水波画在
    // 祖先 Material 上，外面再盖一层不透明底色就把它整个遮住了 —— 那正是之前
    // 「按钮没有水波」的原因，不是没挂 InkWell。
    return Material(
      color: bg,
      animationDuration: pill
          ? const Duration(milliseconds: 200)
          : Duration.zero,
      shape: RoundedRectangleBorder(
        side: BorderSide(color: borderColor),
        borderRadius: radius,
      ),
      child: InkWell(
        onTap: onTap,
        borderRadius: radius,
        child: Padding(
          padding: EdgeInsets.symmetric(
            horizontal: pill ? 12 : 10,
            vertical: pill ? 4 : 5,
          ),
          child: Text(label, style: AidogType.micro.copyWith(color: fg)),
        ),
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
/// 操作结果提示：浮在**窗口顶部居中**，不参与页面滚动。
///
/// 对齐 `src/pages/platforms/PlatformListView.tsx:259-273`：React 是
/// `createPortal` 到 body 的 `position: fixed; top: 24; left: 50%` 彩色胶囊，
/// 带 ✓ / ✕ 图标。
///
/// 原先这里是页面内容里的一条整宽条，**跟着页面一起滚** —— 平台一多，
/// 「已保存」「测试失败」这类提示直接落在屏幕外，用户根本看不到操作结果。
/// 走 `OverlayPortal`（与 [AidogModal] 同一条路）把它抬到根 Overlay 上，
/// 全仓的调用点一行不用改。
///
/// `IgnorePointer`：提示不挡下面的点击（React 那边是 `pointerEvents: none`）。
class ToastBar extends StatefulWidget {
  const ToastBar({super.key, required this.text, required this.ok});

  final String text;
  final bool ok;

  @override
  State<ToastBar> createState() => _ToastBarState();
}

class _ToastBarState extends State<ToastBar> {
  final _controller = OverlayPortalController();

  @override
  void initState() {
    super.initState();
    // 这个 widget 只在「该显示」时才被挂进树，挂上即显示、拔掉即消失。
    // 几秒后自动消失由各页自己的计时器负责（与 React 同），这里不管。
    _controller.show();
  }

  @override
  Widget build(BuildContext context) => OverlayPortal(
    controller: _controller,
    overlayChildBuilder: (context) {
      final theme = AidogTheme.of(context);
      final bg = widget.ok ? theme.c.ok : theme.c.bad;
      return Positioned(
        top: 24,
        left: 0,
        right: 0,
        child: IgnorePointer(
          child: Align(
            alignment: Alignment.topCenter,
            child: Material(
              type: MaterialType.transparency,
              child: Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: AidogSpace.slg,
                  vertical: AidogSpace.ssm,
                ),
                decoration: BoxDecoration(
                  color: bg,
                  borderRadius: BorderRadius.circular(AidogRadius.md),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(
                      widget.ok ? Icons.check : Icons.close,
                      size: 14,
                      // 彩底上的字与图标取浅色模式的 surface 当「白」，
                      // 与 [AidogSwitch] 的圆点同一条路子：不写字面色值。
                      color: AidogColors.light.surface,
                    ),
                    const SizedBox(width: AidogSpace.sxs),
                    Text(
                      widget.text,
                      style: AidogType.micro.copyWith(
                        color: AidogColors.light.surface,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      );
    },
  );
}

/// 输入框：控制器活在 State 里，不随每次 build 重建。
///
/// 这个 widget 存在的唯一理由是修一类反复出现的 bug：
/// 直接写 `TextField(controller: TextEditingController(text: x))`，
/// **每次 build 都造一个新控制器**，于是任何一次外部重建（同页别的东西变了、
/// 列表里增删一行、定时刷新到了）都会把正在输入的内容打回上一次提交的值。
/// MCP 的 KV 编辑器最明显：增删任意一行就会触发。
///
/// 正确做法就是这里的：控制器 `late final` 在 State 里，只有**外部值真的变了**
/// 才同步回输入框（并把光标放到末尾），值没变就不碰，免得打断用户正在选的那一段。
class KeptTextField extends StatefulWidget {
  const KeptTextField({
    super.key,
    required this.value,
    this.onChanged,
    this.onSubmitted,
    this.hint,
    this.keyboardType,
    this.textAlign = TextAlign.start,
    this.maxLines = 1,
  });

  final String value;
  final ValueChanged<String>? onChanged;
  final ValueChanged<String>? onSubmitted;
  final String? hint;
  final TextInputType? keyboardType;
  final TextAlign textAlign;
  final int? maxLines;

  @override
  State<KeptTextField> createState() => _KeptTextFieldState();
}

class _KeptTextFieldState extends State<KeptTextField> {
  late final TextEditingController _ctrl = TextEditingController(
    text: widget.value,
  );
  final FocusNode _focus = FocusNode();

  @override
  void didUpdateWidget(KeptTextField old) {
    super.didUpdateWidget(old);
    // 🔴 `!_focus.hasFocus` 这道闸缺不得。
    //
    // 调用方大多把 `onChanged` 写进一个普通字段（`f.name = v`）**而不触发重建**，
    // 所以用户打字时 `widget.value` 仍停在旧值。只比 `widget.value != _ctrl.text`
    // 的话，下一次外部重建就会拿旧值把用户刚打的字盖掉 —— 换了个形式的同一个 bug。
    // `settings/bits.dart:180` 的 TextRow 早就是这么写的，这里照抄。
    if (widget.value != _ctrl.text && !_focus.hasFocus) {
      _ctrl.value = TextEditingValue(
        text: widget.value,
        selection: TextSelection.collapsed(offset: widget.value.length),
      );
    }
  }

  @override
  void dispose() {
    _focus.dispose();
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = AidogTheme.of(context);
    return TextField(
      controller: _ctrl,
      focusNode: _focus,
      keyboardType: widget.keyboardType,
      textAlign: widget.textAlign,
      maxLines: widget.maxLines,
      decoration: InputDecoration(
        isDense: true,
        hintText: widget.hint,
        hintStyle: AidogType.micro.copyWith(color: theme.c.fg3),
      ),
      style: AidogType.micro.copyWith(color: theme.c.fg),
      onChanged: widget.onChanged,
      onSubmitted: widget.onSubmitted,
    );
  }
}

/// 入场错峰淡入（React 的 `useReveal(delayMs)` + `.reveal` / `.reveal.in`，
/// `globals.css:969-975`）：起始 opacity 0 + 下移 20px，600ms 缓动到位。
///
/// [delayMs] 是这一项相对整批的错峰量（React 侧通常是 `index * 60`）。
/// 系统开了「减少动态效果」时直接到位不动画 —— 与 React 的
/// `@media (prefers-reduced-motion)` 同一条判据（`globals.css:1033`）。
class Reveal extends StatefulWidget {
  const Reveal({super.key, required this.child, this.delayMs = 0});

  final Widget child;
  final int delayMs;

  @override
  State<Reveal> createState() => _RevealState();
}

class _RevealState extends State<Reveal> {
  bool _in = false;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _timer = Timer(Duration(milliseconds: widget.delayMs), () {
      if (mounted) setState(() => _in = true);
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (MediaQuery.disableAnimationsOf(context)) return widget.child;
    return AnimatedSlide(
      offset: _in ? Offset.zero : const Offset(0, 0.25),
      duration: const Duration(milliseconds: 600),
      curve: AidogMotion.easeStandard,
      child: AnimatedOpacity(
        opacity: _in ? 1 : 0,
        duration: const Duration(milliseconds: 600),
        curve: AidogMotion.easeStandard,
        child: widget.child,
      ),
    );
  }
}

/// 悬停抬升 2px（React 的 `.hover-lift`，`globals.css:1004-1008`）。
/// 触屏没有 hover，这个包装在那里是无害的空转。
class HoverLift extends StatefulWidget {
  const HoverLift({super.key, required this.child});

  final Widget child;

  @override
  State<HoverLift> createState() => _HoverLiftState();
}

class _HoverLiftState extends State<HoverLift> {
  bool _over = false;

  @override
  Widget build(BuildContext context) {
    if (MediaQuery.disableAnimationsOf(context)) return widget.child;
    return MouseRegion(
      onEnter: (_) => setState(() => _over = true),
      onExit: (_) => setState(() => _over = false),
      child: AnimatedSlide(
        offset: _over ? const Offset(0, -0.04) : Offset.zero,
        duration: const Duration(milliseconds: 250),
        curve: AidogMotion.easeStandard,
        child: widget.child,
      ),
    );
  }
}

/// 虚线圆角框。Flutter 没有 `border-style: dashed`，自己描一圈。
class DashedBorder extends CustomPainter {
  const DashedBorder({required this.color, required this.radius});

  final Color color;
  final double radius;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = RRect.fromRectAndRadius(
      Offset.zero & size,
      Radius.circular(radius),
    );
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;
    const dash = 6.0;
    const gap = 4.0;
    for (final metric in (Path()..addRRect(rect)).computeMetrics()) {
      var at = 0.0;
      while (at < metric.length) {
        final end = (at + dash).clamp(0.0, metric.length);
        canvas.drawPath(metric.extractPath(at, end), paint);
        at = end + gap;
      }
    }
  }

  @override
  bool shouldRepaint(DashedBorder old) =>
      old.color != color || old.radius != radius;
}
