/// 托盘小窗（票 I11）。主窗口侧只需要 [toggleTrayPanel] / [trayPanelSupported]；
/// 设置页的实时预览用 [PopoverGrid] + [PopoverFrame]（与小窗本体同一份渲染，
/// 单一事实源，避免预览与实际漂移 —— 与 React 的 `renderGrid` 同构）。
///
/// ```dart
/// import 'package:aidog_flutter/popover.dart';
///
/// // 菜单栏图标左键点击（anchor = 图标矩形，左上原点逻辑坐标）
/// await toggleTrayPanel(anchor);
/// ```
///
/// 外壳是 `macos/Runner/TrayPanel.swift` 的 `NSPanel(.nonactivatingPanel)`，
/// 里面装第二个 `FlutterEngine`。**非 macOS 一律并进主窗口**（用户定，
/// `NSPanel` 没有 Windows 对应物），[trayPanelSupported] 是判据。
library;

export 'src/popover/app.dart'
    show PopoverApp, PopoverDataController, runPopoverApp;
export 'src/popover/cards.dart'
    show PopoverCard, PopoverFrame, PopoverGrid, PopoverRoot;
export 'src/popover/model.dart'
    show
        PopoverSize,
        buildPopoverItemQuery,
        buildPopoverTrendQuery,
        kDayMs,
        kPopoverStatsItemTypes,
        kPopoverTodayOnlyTypes,
        normPopoverSize,
        popoverEntryColor,
        popoverItems,
        popoverOverviewTokens,
        popoverRows,
        popoverShareEntries,
        popoverStatsQueries,
        popoverValueColor;
export 'src/popover/panel_channel.dart'
    show
        hideTrayPanel,
        kTrayPanelChannel,
        kTrayPanelContentChannel,
        kTrayPanelMaxHeight,
        kTrayPanelMaxWidth,
        kTrayPanelMinHeight,
        kTrayPanelMinWidth,
        reportTrayPanelSize,
        showTrayPanel,
        toggleTrayPanel,
        trayPanelSupported;
