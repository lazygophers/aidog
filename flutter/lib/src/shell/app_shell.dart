/// A″ 壳骨架（用户 2026-09-24 裁决，推翻票 10 A′ Bento）：复刻 React 玻璃侧栏。
/// **根 padding 12 + gap 12，侧栏浮起玻璃卡，无独立标题栏**（React `App.tsx:197-206`
/// + `Sidebar.tsx:250-260`）。内容区 main padding 24/32 + radius-lg（`App.tsx:212-218`）。
/// 每一页只换格子内容，不换骨架。
///
/// 窗口拖拽：macOS 原生标题栏（`MainMenu.xib` titled 窗口）承担，壳内没有也不需要拖拽热区。
library;

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

import '../../i18n.dart' show kSupportedFlutterLocales;

import 'nav.dart';
import 'rail.dart';
import 'theme.dart';

/// 页头：大标题 + 一句话副标题 + 右侧的时间/维度切换。
class PageHead extends StatelessWidget {
  const PageHead({
    super.key,
    required this.title,
    this.subtitle,
    this.trailing,
    this.bottom,
    this.titleStyle,
    this.subtitleStyle,
    this.inlineSubtitle = false,
    this.leading,
  });

  /// 标题**左边**的控件。Skills 安装子视图的「← 返回」排在标题左侧
  /// （`SkillInstallView.tsx:225-238`），不是右侧操作区。
  final Widget? leading;

  final String title;
  final String? subtitle;
  final Widget? trailing;

  /// 与后续内容的间距。缺省 2xl；React 页面全页 gap（如 Stats 的 16）经这里传入。
  final double? bottom;

  /// 标题字阶覆盖。缺省 `.section-title` = 18 w700 ls-0.02em
  /// （`src/styles/globals.css:702-707`，Home / Stats / Logs / About /
  /// 平台页 / RequestLog 六页共用这一个类）；React 各页头另有不同档：
  /// Skills 18 w700（`SkillsView.tsx:58`）、MCP 22 w700（`McpView.tsx:21`）、
  /// 安装子视图 18 w700（`SkillInstallView.tsx:236`）。
  final TextStyle? titleStyle;

  /// 副标题字阶覆盖。缺省 `.section-desc` = 13 secondary
  /// （`globals.css:709-712`）。
  final TextStyle? subtitleStyle;

  /// 副标题排在标题**同一行右侧**（MCP 的计数、Skills 的「刷新中…」都是这形态，
  /// `McpView.tsx:24-26`、`SkillsView.tsx:59-63`）。缺省 false = 排在标题下一行。
  final bool inlineSubtitle;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final sub = subtitle;
    final subText = sub == null
        ? null
        : Text(
            sub,
            style:
                subtitleStyle ??
                AidogType.caption.copyWith(fontSize: 13, color: t.c.fg2),
          );
    final titleText = Text(
      title,
      style:
          (titleStyle ??
                  AidogType.display.copyWith(
                    fontSize: 18,
                    fontWeight: FontWeight.w700,
                    letterSpacing: -0.36,
                  ))
              .copyWith(color: t.c.fg),
    );
    return Padding(
      padding: EdgeInsets.only(bottom: bottom ?? AidogSpace.s_2xl),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          if (leading != null) ...[
            leading!,
            // React 这一组的 gap 是 10（`SkillInstallView.tsx:225`）。
            const SizedBox(width: AidogSpace.smd),
          ],
          Expanded(
            child: inlineSubtitle
                ? Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Flexible(child: titleText),
                      if (subText != null) ...[
                        const SizedBox(width: AidogSpace.smd),
                        Flexible(child: subText),
                      ],
                    ],
                  )
                : Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                titleText,
                if (subText != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 3),
                    child: subText,
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

/// 「活着」的圆点：ok 色 + live-ring + 3s 呼吸。熄灭时用 fg-3，不发光。
/// （设置页的运行状态卡用它；壳骨架已无标题栏，这里只剩这个消费点。）
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

/// 整个外壳。页面票只提供 [pageBuilder]：拿到 activeId，返回那一页的内容。
class AppShell extends StatefulWidget {
  const AppShell({
    super.key,
    required this.controller,
    required this.theme,
    required this.pageBuilder,
    this.t = _identity,
    this.localeLabel = '',
    this.onPickLocale,
    this.locales = const [],
    this.onSelectLocale,
    this.initialCollapsed = false,
  });

  final ShellController controller;
  final ThemeController theme;
  final Widget Function(BuildContext context, String activeId) pageBuilder;
  final String Function(String key) t;

  final String localeLabel;
  final VoidCallback? onPickLocale;

  /// 语言下拉的候选与选中回调（React `Sidebar.tsx:520-531` 的 `ALL_LOCALES`
  /// + `setLocale`）。透传给 [Rail]；不传就退回 [onPickLocale]。
  final List<String> locales;
  final void Function(String locale)? onSelectLocale;

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

/// 窗口底：纯色 + 主色光晕。
///
/// 对齐 React 的 `--app-bg-overlay`（`src/themes/mono.ts:41-46`）：底色之上再叠
/// 两到三层从 accent 派生的 radial-gradient。原先这里只有一层 `ColoredBox`，
/// 于是 Flutter 的窗口底是一块纯色，而 React 顶部偏亮带主色晕 —— 两版并排时
/// 这是最先被看出来的一处差别。
///
/// 画在骨架这一层，不在各页里各画一次：它是窗口底，不属于任何一页。
class _WindowBackground extends StatelessWidget {
  const _WindowBackground({required this.theme, required this.child});

  final AidogTheme theme;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final overlays = bgOverlays(theme.c);
    return ColoredBox(
      color: theme.c.bg,
      child: Stack(
        children: [
          for (final g in overlays)
            Positioned.fill(
              child: IgnorePointer(
                child: DecoratedBox(decoration: BoxDecoration(gradient: g)),
              ),
            ),
          // 侧栏↔内容的分隔光晕：React 是根容器的
          // `linear-gradient(90deg, transparent 200px, primary 8% 212px, transparent 224px)`
          //（`App.tsx:203-205`）。200 正是 railW，带子落在轨与内容之间。
          // `--primary` = `c.accent`（`mono.ts:56`）；此处沿用本批次 #72 / #77 的
          // 裁决，用 accentText（暗色 accent 近黑，8% 叠在暗底上看不见）。
          PositionedDirectional(
            start: AidogLayout.railW,
            top: 0,
            bottom: 0,
            width: 24,
            child: IgnorePointer(
              child: DecoratedBox(
                decoration: BoxDecoration(
                  gradient: LinearGradient(
                    begin: AlignmentDirectional.centerStart,
                    end: AlignmentDirectional.centerEnd,
                    colors: [
                      theme.c.accentText.withValues(alpha: 0),
                      theme.c.accentText.withValues(alpha: 0.08),
                      theme.c.accentText.withValues(alpha: 0),
                    ],
                  ),
                ),
              ),
            ),
          ),
          child,
        ],
      ),
    );
  }
}

/// 页级内容宽上限。`AidogLayout.contentMax`(1180) 是壳的缺省，React 侧各页
/// 自带的值不一样：Home 是 `maxWidth: 1200`（`Home.tsx:265`），Stats / Logs /
/// RequestLog **不设上限**（`Stats.tsx:477`、`Logs/ListView.tsx:56`、
/// `RequestLog.tsx:193`）。
double _contentMaxFor(String activeId) => switch (activeId.split('/').first) {
  'home' => 1200,
  'stats' || 'logs' || 'request-log' => double.infinity,
  _ => AidogLayout.contentMax,
};

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
        return _WindowBackground(
          theme: t,
          // 整个骨架统一给一层 Material（Scaffold 干的就是这件事，而这里自绘不走 Scaffold）。
          // 少了它，侧栏的每一行文字都会被 Flutter 画上「缺 Material 祖先」的黄色下划线。
          // 透明色：底色仍由上面的 bg token 决定，Material 只负责提供墨层与默认文字样式。
          child: PageStickyHeader(
            slot: _sticky,
            child: Material(
              type: MaterialType.transparency,
              // React 根容器：padding 12 + gap 12（App.tsx:197-206），侧栏与内容
              // 都是浮在窗口底上的玻璃卡。
              child: Padding(
                padding: const EdgeInsets.all(AidogLayout.shellInset),
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
                      locales: widget.locales,
                      onSelectLocale: widget.onSelectLocale,
                      t: widget.t,
                    ),
                    const SizedBox(width: AidogLayout.shellGap),
                    Expanded(
                      // React main：padding 24/32 + radius-lg（App.tsx:212-218）。
                      // main 本身无底色，radius 只裁滚动内容的角 —— ClipRRect 同理。
                      child: ClipRRect(
                        borderRadius: BorderRadius.circular(AidogRadius.lg),
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
                                            horizontal: AidogLayout.pagePadX,
                                          ),
                                          child: sticky,
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              SliverPadding(
                                padding: EdgeInsets.fromLTRB(
                                  AidogLayout.pagePadX,
                                  // 横条在时它自己占掉了顶部留白。
                                  sticky == null ? AidogLayout.pagePadY : 0,
                                  AidogLayout.pagePadX,
                                  AidogLayout.pagePadY,
                                ),
                                sliver: SliverToBoxAdapter(
                                  child: Center(
                                    child: ConstrainedBox(
                                      constraints: BoxConstraints(
                                        maxWidth: _contentMaxFor(
                                          widget.controller.activeId,
                                        ),
                                      ),
                                      // Material 已由骨架根部统一提供（见上），
                                      // 这里不再包第二层。
                                      // 切页淡入：React 是
                                      // `<div className="animate-fade-in" key={nav}>`
                                      //（`App.tsx:222`），每次换页重播。
                                      child: AnimatedSwitcher(
                                        duration: AidogMotion.base,
                                        child: KeyedSubtree(
                                          key: ValueKey(
                                            widget.controller.activeId,
                                          ),
                                          child: widget.pageBuilder(
                                            context,
                                            widget.controller.activeId,
                                          ),
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
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
    this.localeLabel = '',
    this.onPickLocale,
    this.locales = const [],
    this.onSelectLocale,
    this.textDirection = TextDirection.ltr,
    this.locale,
  });

  final ShellController controller;
  final ThemeController theme;
  final Widget Function(BuildContext context, String activeId) pageBuilder;
  final String Function(String key) t;
  final String localeLabel;
  final VoidCallback? onPickLocale;

  /// 透传给 [AppShell] → [Rail] 的语言下拉数据。
  final List<String> locales;
  final void Function(String locale)? onSelectLocale;

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
          localeLabel: localeLabel,
          onPickLocale: onPickLocale,
          locales: locales,
          onSelectLocale: onSelectLocale,
        ),
      ),
    );
  }
}
