/// 页面层入口。`main.dart` 的 `pageBuilder` 与页面票（I06-I09）只从这里 import。
///
/// ```dart
/// import 'package:aidog_flutter/pages.dart';
///
/// pageBuilder: (ctx, id) => switch (id) {
///   'home' => HomePage(onNavigate: nav.navigate),
///   'stats' => const StatsPage(),
///   _ => ...,
/// },
/// ```
///
/// 两条约定（票 I06 定，后续页面票沿用）：
/// 1. **没有第二套 api 封装**。页面直接 `kernel.invoke` 传内联 map，字段名从
///    `src/services/api/types/generated/<Type>.ts` 抄。理由见 flutter/README.md
///    「不生成 203 个 Dart 绑定」。
/// 2. **每个页面收一个 [InvokeFn]**（缺省 [kernelInvoke]），widget 测试塞假实现 ——
///    全局 `kernel` 是 `final`，不留这个口子就没法在不起内核的情况下测页面。
library;

export 'src/pages/filter_dropdown.dart' show FilterDropdown, FilterOption, filterOptions;
export 'src/pages/groups_logic.dart';
export 'src/pages/home.dart' show HomePage;
export 'src/pages/home_logic.dart';
export 'src/pages/invoke.dart'
    show InvokeFn, debounceStream, kProxyLogUpdatedEvent, kernelInvoke, kernelProxyLogUpdated;
export 'src/pages/groups.dart' show GroupsSection;
export 'src/pages/logs.dart' show LogsPage, RequestLogPage;
export 'src/pages/logs_logic.dart';
export 'src/pages/models.dart';
export 'src/pages/platforms.dart' show PlatformsPage;
// 票 I16：设置 12 个子页的 widget 层（`settings/pricing` 的模型信息页属票 I09）。
export 'src/pages/settings/bits.dart'
    show
        AutoToast,
        ChoiceRow,
        ErrorNote,
        InfoRow,
        NumberRow,
        SelectRow,
        SettingsCard,
        SettingsPageBody,
        SwitchRow,
        TextRow,
        UnsavedChangesCard,
        tOr;
export 'src/pages/settings/coding_tools_page.dart' show CodingToolsPage;
export 'src/pages/settings/importexport_page.dart' show ImportExportPage;
export 'src/pages/settings/mitm_page.dart' show MitmSettingsPage;
export 'src/pages/settings/notification_events.dart';
export 'src/pages/settings/notifications_page.dart'
    show NotificationsSettingsPage;
export 'src/pages/settings/rules_pages.dart'
    show MiddlewareSettingsPage, SchedulingSettingsPage;
export 'src/pages/settings/schema_config_page.dart'
    show
        ImportDiffCard,
        SchemaBundle,
        SchemaConfigKind,
        SchemaConfigPage,
        SchemaField,
        SchemaSection,
        loadClaudeLanguageOptions,
        loadSchemaBundle;
export 'src/pages/settings/system_page.dart' show SystemSettingsPage;
export 'src/pages/settings/tray_pages.dart'
    show PopoverSettingsPage, TraySettingsPage;
export 'src/pages/platforms_logic.dart';
export 'src/pages/stats.dart' show StatsPage;
export 'src/pages/stats_logic.dart';
export 'src/pages/ui_bits.dart'
    show CenteredNote, ConfirmCard, SmallButton, ToastBar;
