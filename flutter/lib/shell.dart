/// aidog 主题 + A′ 骨架 + 导航 + 四种格子。页面票（I06-I09）、托盘（I11）、
/// 浏览器版（I14）只从这里 import。
///
/// ```dart
/// import 'package:aidog_flutter/shell.dart';
///
/// AidogShellApp(
///   controller: ShellController(),      // 导航；navigate() 自动过离页拦截
///   theme: ThemeController(),           // 默认深色，手动切，不跟随系统
///   pageBuilder: (ctx, id) => switch (id.split('/').first) {
///     'home' => const HomePage(),
///     _ => const SizedBox.shrink(),
///   },
/// );
/// ```
///
/// 三条不能破的：
/// 1. **色值只来自 token 表**（`AidogColors` / `AidogTheme.of(context).c`）。
///    改色请改 `design/tokens/tokens.json` 后跑 `yarn tokens`，`yarn check:tokens` 会拦。
/// 2. **只有四种格子**：[ReadoutTile] / [SeriesTile] / [ListingTile] / [ActionTile]，
///    排进 [Bento]。不发明第五种。
/// 3. **切页一律走 [ShellController.navigate]**，它内部过 [requestNavigation] ——
///    脏表单靠 [registerNavGuard] 拦住，绕过去就会静默丢用户的编辑。
library;

export 'src/shell/app_shell.dart'
    show AidogShellApp, AppShell, LiveDot, PageHead, PageStickyHeader, Titlebar;
export 'src/shell/nav.dart'
    show NavChild, NavContext, NavItem, ShellController, groupAdjacent, kBaseNav;
export 'src/shell/nav_guard.dart'
    show NavGuard, hasNavGuard, registerNavGuard, requestNavigation, resetNavGuardForTest;
export 'src/shell/rail.dart' show Rail, kNavIcons;
export 'src/shell/theme.dart'
    show
        AidogColors,
        AidogLayout,
        AidogMode,
        AidogMotion,
        AidogRadius,
        AidogSpace,
        AidogTheme,
        AidogType,
        ThemeController,
        aidogThemeData,
        bgOverlays,
        parseShadow;
export 'src/shell/tiles.dart'
    show
        ActionTile,
        Bento,
        BentoCell,
        ListingTile,
        Ltr,
        ReadoutTile,
        SeriesTile,
        Tile,
        TileMeta,
        Trend,
        effectiveSpan,
        numStyle;
