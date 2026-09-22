/// A′ 骨架：**标题栏 40px 通栏 → 其下是「侧栏 200px（可折叠 56px） + 内容区」**。
/// 每一页只换格子内容，不换骨架，也**不再有第二套导航**（票 10 布局规则第 1 条）。
///
/// 内容区 = 12 列 bento（[Bento]）、gap 10、page-pad 18、max-width 1180 居中。
library;

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

import '../../i18n.dart' show kSupportedFlutterLocales;

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
          if (leading != null) ...[
            leading!,
            const SizedBox(width: AidogSpace.smd),
          ],
          // 品牌标，对应 React `Sidebar.tsx:268-277` 的 `<img src="/logo.svg">`。
          // 资产由 `node scripts/gen-flutter-icons.mjs` 从 `src-tauri/icons/` 同步，
          // 与 Tauri 壳同一个真值源；漂了 `--check` 会红。
          Image.asset(
            'assets/logo.webp',
            width: 18,
            height: 18,
            filterQuality: FilterQuality.medium,
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
              child: Text(status!, style: numStyle(AidogType.numSm, t.c.fg2)),
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
  const PageHead({
    super.key,
    required this.title,
    this.subtitle,
    this.trailing,
  });

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

/// 页面往骨架的滚动视口顶部挂一条**粘顶**的横条（设置页的 section 跳转条就是它）。
///
/// 为什么要骨架出面：滚动视口在骨架里（下面那个 [CustomScrollView]），粘顶只能由
/// 视口自己的 sliver 实现，页面在视口内部做不到。页面把要粘的 widget 写进这个
/// notifier，骨架把它渲染成 `SliverPersistentHeader(pinned: true)`。
///
/// 拿不到（widget 测试里单独挂页面、没有骨架）→ `maybeOf` 返回 null，
/// 页面自行退回「页内一行」的形态。
class PageStickyHeader extends InheritedWidget {
  const PageStickyHeader({super.key, required this.slot, required super.child});

  final ValueNotifier<Widget?> slot;

  static ValueNotifier<Widget?>? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<PageStickyHeader>()?.slot;

  @override
  bool updateShouldNotify(PageStickyHeader oldWidget) =>
      !identical(slot, oldWidget.slot);
}

/// 粘顶横条的高度。固定值而不是量内容：`SliverPersistentHeader` 要求先给高度，
/// 而这条横条只装一行 pill 按钮，高度本来就是定的。
const double kStickyHeaderH = 46;

class _StickyHeaderDelegate extends SliverPersistentHeaderDelegate {
  _StickyHeaderDelegate({required this.child, required this.background});

  final Widget child;
  final Color background;

  @override
  double get minExtent => kStickyHeaderH;

  @override
  double get maxExtent => kStickyHeaderH;

  @override
  Widget build(BuildContext context, double shrinkOffset, bool overlaps) =>
      // 不透明底色是必需的：内容会从它底下滚过去，透明的话两层字会叠在一起。
      ColoredBox(color: background, child: child);

  @override
  bool shouldRebuild(_StickyHeaderDelegate old) =>
      old.child != child || old.background != background;
}

class _AppShellState extends State<AppShell> {
  late bool _collapsed = widget.initialCollapsed;

  /// 当前页挂上来的粘顶横条（没有就是 null）。
  final ValueNotifier<Widget?> _sticky = ValueNotifier(null);

  @override
  void dispose() {
    _sticky.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return AnimatedBuilder(
      animation: Listenable.merge([widget.controller, widget.theme]),
      builder: (context, _) {
        return ColoredBox(
          color: t.c.bg,
          // 整个骨架统一给一层 Material（Scaffold 干的就是这件事，而这里自绘不走 Scaffold）。
          // 少了它，顶栏与侧栏的每一行文字都会被 Flutter 画上「缺 Material 祖先」的黄色下划线
          // —— 页面区之前单独包过一层，所以只有那一块是干净的，看起来像设计差异，其实是缺层。
          // 透明色：底色仍由上面的 bg token 决定，Material 只负责提供墨层与默认文字样式。
          child: PageStickyHeader(
            slot: _sticky,
            child: Material(
              type: MaterialType.transparency,
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
                          // 从 SingleChildScrollView 换成 CustomScrollView：页面仍是
                          // 一整个 box（装在 SliverToBoxAdapter 里，写页面的方式不变），
                          // 换的目的只有一个 —— 让页面能往视口顶部挂一条粘住的横条。
                          child: ValueListenableBuilder<Widget?>(
                            valueListenable: _sticky,
                            builder: (context, sticky, _) => CustomScrollView(
                              slivers: [
                                if (sticky != null)
                                  SliverPersistentHeader(
                                    pinned: true,
                                    delegate: _StickyHeaderDelegate(
                                      background: t.c.bg,
                                      child: Center(
                                        child: ConstrainedBox(
                                          constraints: const BoxConstraints(
                                            maxWidth: AidogLayout.contentMax,
                                          ),
                                          child: Padding(
                                            padding: const EdgeInsets.symmetric(
                                              horizontal: AidogLayout.pagePad,
                                            ),
                                            child: sticky,
                                          ),
                                        ),
                                      ),
                                    ),
                                  ),
                                SliverPadding(
                                  padding: EdgeInsets.fromLTRB(
                                    AidogLayout.pagePad,
                                    // 横条在时它自己占掉了顶部留白。
                                    sticky == null ? AidogLayout.pagePad : 0,
                                    AidogLayout.pagePad,
                                    40,
                                  ),
                                  sliver: SliverToBoxAdapter(
                                    child: Center(
                                      child: ConstrainedBox(
                                        constraints: const BoxConstraints(
                                          maxWidth: AidogLayout.contentMax,
                                        ),
                                        // Material 已由骨架根部统一提供（见上），
                                        // 这里不再包第二层。
                                        child: widget.pageBuilder(
                                          context,
                                          widget.controller.activeId,
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
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
    this.locale,
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

  /// Material / Cupertino 自带控件（文本选择菜单、日期选择器等）的语言。
  /// 应用自己的文案走 [t]，与这个无关。
  final Locale? locale;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: theme,
      builder: (context, _) => MaterialApp(
        title: 'aidog',
        debugShowCheckedModeBanner: false,
        theme: theme.data,
        themeMode: ThemeMode.light,
        locale: locale,
        supportedLocales: kSupportedFlutterLocales,
        localizationsDelegates: const <LocalizationsDelegate<Object>>[
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
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
