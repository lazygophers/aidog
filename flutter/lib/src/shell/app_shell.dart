/// A′ 骨架：**标题栏 40px 通栏 → 其下是「侧栏 200px（可折叠 56px） + 内容区」**。
/// 每一页只换格子内容，不换骨架，也**不再有第二套导航**（票 10 布局规则第 1 条）。
///
/// 内容区 = 12 列 bento（[Bento]）、gap 10、page-pad 18、max-width 1180 居中。
library;

import 'package:flutter/material.dart';

import 'nav.dart';
import 'rail.dart';
import 'theme.dart';
import 'tiles.dart';

/// 标题栏：通栏 40px，左侧品牌，右侧「活着」的状态点（呼吸）。
class Titlebar extends StatelessWidget {
  const Titlebar({
    super.key,
    required this.title,
    this.status,
    this.live = false,
    this.leading,
  });

  final String title;

  /// 形如 `127.0.0.1:8787 · 运行中`；URL / 端口是规则 6 第 ③ 类显式 LTR。
  final String? status;

  /// 代理是否在跑 —— 只有活着的东西才发光。
  final bool live;
  final Widget? leading;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Container(
      height: AidogLayout.titlebarH,
      padding: const EdgeInsets.symmetric(horizontal: AidogSpace.slg),
      decoration: BoxDecoration(
        color: t.c.bgChrome,
        border: Border(bottom: BorderSide(color: t.c.line)),
      ),
      child: Row(
        children: [
          if (leading != null) ...[leading!, const SizedBox(width: AidogSpace.smd)],
          Container(
            width: 18,
            height: 18,
            decoration: BoxDecoration(
              color: t.c.accent,
              borderRadius: BorderRadius.circular(AidogRadius.sm),
            ),
          ),
          const SizedBox(width: 7),
          Text(
            title,
            style: AidogType.title.copyWith(color: t.c.fg, fontSize: 13),
          ),
          const Spacer(),
          if (status != null) ...[
            LiveDot(on: live),
            const SizedBox(width: 7),
            Ltr(
              child: Text(
                status!,
                style: numStyle(AidogType.numSm, t.c.fg2),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// 「活着」的圆点：ok 色 + live-ring + 3s 呼吸。熄灭时用 fg-3，不发光。
class LiveDot extends StatefulWidget {
  const LiveDot({super.key, required this.on, this.size = 7});

  final bool on;
  final double size;

  @override
  State<LiveDot> createState() => _LiveDotState();
}

class _LiveDotState extends State<LiveDot> with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: AidogMotion.breathe,
  )..repeat(reverse: true);

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final dot = Container(
      width: widget.size,
      height: widget.size,
      decoration: BoxDecoration(
        color: widget.on ? t.c.ok : t.c.fg3,
        shape: BoxShape.circle,
        boxShadow: widget.on ? t.liveRing : null,
      ),
    );
    if (!widget.on) return dot;
    // 深色是阴影扩散，浅色是不透明度 —— 两套都由 opacity 表达，token 决定有没有 halo。
    return FadeTransition(
      opacity: Tween<double>(begin: 1, end: 0.45).animate(_c),
      child: dot,
    );
  }
}

/// 页头：大标题 + 一句话副标题 + 右侧的时间/维度切换。
class PageHead extends StatelessWidget {
  const PageHead({super.key, required this.title, this.subtitle, this.trailing});

  final String title;
  final String? subtitle;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: AidogSpace.s_2xl),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(title, style: AidogType.display.copyWith(color: t.c.fg)),
                if (subtitle != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 3),
                    child: Text(
                      subtitle!,
                      style: AidogType.caption.copyWith(color: t.c.fg2),
                    ),
                  ),
              ],
            ),
          ),
          if (trailing != null) ...[
            const SizedBox(width: AidogSpace.s_2xl),
            trailing!,
          ],
        ],
      ),
    );
  }
}

/// 整个外壳。页面票只提供 [pageBuilder]：拿到 activeId，返回那一页的内容。
class AppShell extends StatefulWidget {
  const AppShell({
    super.key,
    required this.controller,
    required this.theme,
    required this.pageBuilder,
    this.t = _identity,
    this.appTitle = 'aidog',
    this.status,
    this.live = false,
    this.localeLabel = '',
    this.onPickLocale,
    this.initialCollapsed = false,
  });

  final ShellController controller;
  final ThemeController theme;
  final Widget Function(BuildContext context, String activeId) pageBuilder;
  final String Function(String key) t;
  final String appTitle;
  final String? status;
  final bool live;
  final String localeLabel;
  final VoidCallback? onPickLocale;

  /// 侧栏**默认展开 200px**（票 10 用户 2026-09-18 定）；折叠态 56px 也在。
  final bool initialCollapsed;

  static String _identity(String k) => k;

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  late bool _collapsed = widget.initialCollapsed;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return AnimatedBuilder(
      animation: Listenable.merge([widget.controller, widget.theme]),
      builder: (context, _) {
        return ColoredBox(
          color: t.c.bg,
          child: Column(
            children: [
              Titlebar(
                title: widget.appTitle,
                status: widget.status,
                live: widget.live,
              ),
              Expanded(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Rail(
                      items: widget.controller.items,
                      activeId: widget.controller.activeId,
                      onNavigate: widget.controller.navigate,
                      collapsed: _collapsed,
                      onToggleCollapsed: () =>
                          setState(() => _collapsed = !_collapsed),
                      isDark: widget.theme.isDark,
                      onToggleTheme: widget.theme.toggle,
                      localeLabel: widget.localeLabel,
                      onPickLocale: widget.onPickLocale ?? () {},
                      t: widget.t,
                    ),
                    Expanded(
                      child: SingleChildScrollView(
                        padding: const EdgeInsets.fromLTRB(
                          AidogLayout.pagePad,
                          AidogLayout.pagePad,
                          AidogLayout.pagePad,
                          40,
                        ),
                        child: Center(
                          child: ConstrainedBox(
                            constraints: const BoxConstraints(
                              maxWidth: AidogLayout.contentMax,
                            ),
                            // 骨架自绘，不走 Scaffold，所以页面区上方没有 Material。
                            // TextField / Checkbox / InkWell 这类 material 组件都要求
                            // 祖先里有一个，由骨架统一给一次 —— 否则 22 个页面各包一层。
                            // 透明色：底色仍由骨架的 bg token 决定，Material 只提供墨层。
                            child: Material(
                              type: MaterialType.transparency,
                              child: widget.pageBuilder(
                                context,
                                widget.controller.activeId,
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// 把主题、导航、壳一次装好的根 widget。**themeMode 恒为 light** —— 深浅由
/// [ThemeController] 给的 ThemeData 决定，不跟随系统。
class AidogShellApp extends StatelessWidget {
  const AidogShellApp({
    super.key,
    required this.controller,
    required this.theme,
    required this.pageBuilder,
    this.t = AppShell._identity,
    this.status,
    this.live = false,
    this.localeLabel = '',
    this.onPickLocale,
    this.textDirection = TextDirection.ltr,
  });

  final ShellController controller;
  final ThemeController theme;
  final Widget Function(BuildContext context, String activeId) pageBuilder;
  final String Function(String key) t;
  final String? status;
  final bool live;
  final String localeLabel;
  final VoidCallback? onPickLocale;

  /// 票 I03 接 8 语言时把阿拉伯语切成 rtl。
  final TextDirection textDirection;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: theme,
      builder: (context, _) => MaterialApp(
        title: 'aidog',
        debugShowCheckedModeBanner: false,
        theme: theme.data,
        themeMode: ThemeMode.light,
        builder: (context, child) => Directionality(
          textDirection: textDirection,
          child: child ?? const SizedBox.shrink(),
        ),
        home: AppShell(
          controller: controller,
          theme: theme,
          pageBuilder: pageBuilder,
          t: t,
          status: status,
          live: live,
          localeLabel: localeLabel,
          onPickLocale: onPickLocale,
        ),
      ),
    );
  }
}
