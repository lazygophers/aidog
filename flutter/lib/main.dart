/// 主窗口入口。骨架、主题、导航全在 `shell.dart`，文案与方向全在 `i18n.dart`；
/// 页面内容是票 I06-I09 的活，这里先按 activeId 占位，页面票逐个替换 `_placeholder`。
library;

import 'dart:async';

import 'package:app_links/app_links.dart';
import 'package:flutter/material.dart';

import 'i18n.dart';
import 'pages.dart';
import 'popover.dart';
import 'shell.dart';
import 'src/deep_link.dart';
import 'src/menubar.dart';
import 'src/updater.dart';
import 'transport.dart';

/// 托盘小窗那个引擎的入口（票 I11）。
///
/// macOS 的 `FlutterEngine.run(withEntrypoint:)` 只在**默认库**（即 `lib/main.dart`）
/// 里找同名顶层函数，所以这一行转发不能挪到 `src/popover/app.dart` 去。
/// `@pragma('vm:entry-point')` 挡住 AOT 的摇树 —— 没有 Dart 侧调用者，删了就是
/// release 包里找不到入口、小窗白屏。
@pragma('vm:entry-point')
Future<void> popoverMain() => runPopoverApp();

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // 票 I13：桌面壳的自动更新（Sparkle / WinSparkle）。放 runApp 之前、不 await 拖首帧。
  unawaited(initDesktopUpdater());
  // 文案是构建期资产，不依赖内核 —— 所以「后端连接中」这一屏本身就是翻好的。
  await i18n.init();
  runApp(const AidogI18n(child: AidogApp()));
  // 内核起来之后再读用户在后端存的语言设置，覆盖掉按系统猜的那个。
  kernel
      .start()
      .then((_) async {
        await i18n.loadFromBackend();
        // 票 I20：菜单栏的第一帧。图标本身在 Swift 那边启动即建，这里只补数据，
        // 所以它失败也不影响菜单栏能点。
        if (menuBarSupported) await menuBar.start();
      })
      .catchError((Object e) {
        debugPrint('kernel start failed: $e');
      });
}

class AidogApp extends StatefulWidget {
  const AidogApp({super.key});

  @override
  State<AidogApp> createState() => _AidogAppState();
}

class _AidogAppState extends State<AidogApp> {
  final _nav = ShellController();
  final _theme = ThemeController();
  StreamSubscription<Uri>? _deepLinkSub;

  @override
  void initState() {
    super.initState();
    unawaited(_initDeepLinks());
  }

  /// `aidog://<entity>/<action>?data=` 深链（补核 A）：app_links 的
  /// `uriLinkStream` 同时抛冷启动首条与运行时唤起，解析后送 [deepLinks] 总线并
  /// 跳到目标页。对应 React 侧 Rust `deep_link.rs`（URL →
  /// `{entity, action, data}` → emit）+ `App.tsx:123-143` 的分发。
  /// scheme 注册在 bundle 期（macOS Info.plist 的 CFBundleURLTypes），
  /// dev 模式未打包二进制里点 `aidog://` 不唤起本实例 —— 与 Tauri 同一限制。
  Future<void> _initDeepLinks() async {
    final links = AppLinks();
    // app_links README：这条 stream 包括 initial link + 后续事件；另调
    // getInitialLink 会把冷启动链接消费两遍。
    _deepLinkSub = links.uriLinkStream.listen(
      _handleDeepLink,
      onError: (Object e) => debugPrint('deep-link stream error: $e'),
    );
  }

  void _handleDeepLink(Uri uri) {
    final payload = parseDeepLink(uri);
    if (payload == null) {
      debugPrint('deep-link: skipped malformed/non-aidog url $uri');
      return;
    }
    deepLinks.dispatch(payload);
    // 跳到目标页触发挂载（React `App.tsx:138-140` 的 setActiveNav）。
    final page = switch (payload.entity) {
      'platform' => 'platforms',
      'mcp' => 'mcp',
      'skill' => 'skills',
      _ => null,
    };
    if (page != null) _nav.navigate(page);
  }

  @override
  void dispose() {
    _deepLinkSub?.cancel();
    _nav.dispose();
    _theme.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // i18n 是 ChangeNotifier：切语言要连带换 textDirection 与整棵树的文案。
    return AnimatedBuilder(
      animation: i18n,
      builder: (context, _) => StreamBuilder<KernelState>(
        stream: kernel.states,
        initialData: kernel.state,
        builder: (context, snap) {
          final connected = snap.data == KernelState.connected;
          return AidogShellApp(
            controller: _nav,
            theme: _theme,
            t: i18n.t,
            locale: i18n.flutterLocale,
            textDirection: i18n.textDirection,
            localeLabel: i18n.locale,
            live: connected,
            // 地址是内部标识，不翻译，也不该被 bidi 重排。
            status: connected
                ? ltr(
                    '${kernel.process.address?.host}:'
                    '${kernel.process.address?.port}',
                  )
                : i18n.t('common.loading'),
            pageBuilder: _page,
          );
        },
      ),
    );
  }

  /// 票 I06 起逐页替换 [_placeholder]：已落地的走真页面，其余仍是占位。
  ///
  /// 票 I16 接进了设置的 12 个子页。`settings/pricing`（模型信息）属票 I09，
  /// 那一页落地前仍走占位。
  Widget _page(BuildContext context, String id) => switch (id) {
    'home' => HomePage(onNavigate: _nav.navigate),
    // 跨页预筛（React `App.tsx:224-228` 的 `initialFilter={navContext}`）：
    // 平台卡「查看日志」带 platformId，分组卡「查看统计」带 groupKey。
    // `navigate` 每次都会把 _context 重置成空（`nav.dart:174`），所以从侧栏正常
    // 切页不会把上一次的筛选带回来，不需要另做「消费后清空」。
    'stats' => StatsPage(
      initialPlatformId: _nav.context.platformId,
      initialGroupKey: _nav.context.groupKey,
    ),
    // 票 I07：平台页内嵌分组区（与 React 的 GroupsEmbedded 同结构）。
    'platforms' => PlatformsPage(
      onNavigate: (to, {int? platformId, String? groupKey}) => _nav.navigate(
        to,
        NavContext(platformId: platformId, groupKey: groupKey),
      ),
    ),
    'logs' => LogsPage(
      initialPlatformId: _nav.context.platformId,
      initialGroupKey: _nav.context.groupKey,
    ),
    'request-log' => const RequestLogPage(),
    // 裸 `settings` 回退 system，与 `nav.dart::settingsTab` 同规则。
    'settings' || 'settings/system' => const SystemSettingsPage(),
    'settings/coding_tools' => const CodingToolsPage(),
    'settings/claude' => const SchemaConfigPage(kind: SchemaConfigKind.claude),
    'settings/codex' => const SchemaConfigPage(kind: SchemaConfigKind.codex),
    'settings/pi' => const SchemaConfigPage(kind: SchemaConfigKind.pi),
    'settings/middleware' => const MiddlewareSettingsPage(),
    'settings/scheduling' => const SchedulingSettingsPage(),
    'settings/notifications' => const NotificationsSettingsPage(),
    'settings/tray' => const TraySettingsPage(),
    'settings/popover' => const PopoverSettingsPage(),
    'settings/importexport' => const ImportExportPage(),
    'settings/mitm' => const MitmSettingsPage(),
    // 票 I09：技能 / MCP / 通知中心 / 关于 / 模型信息（设置里的「模型信息」子页）。
    'skills' => const SkillsPage(),
    'mcp' => const McpPage(),
    'notifications' => NotificationsPage(onNavigate: _nav.navigate),
    // 桌面形态给检查按钮（Sparkle 接管后续流程）；其余形态维持说明分支。
    'about' => AboutPage(
      onCheckUpdate: desktopUpdaterSupported ? checkForAppUpdates : null,
    ),
    'settings/pricing' => const ModelInfoPage(),
    _ => _placeholder(context, id),
  };

  Widget _placeholder(BuildContext context, String id) {
    final t = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHead(
          title: i18n.t('nav.${id.split('/').first}'),
          subtitle: ltr(id),
        ),
        Bento(
          children: [
            BentoCell(
              span: 12,
              child: Tile(
                child: Text(
                  '页面内容由票 I06-I09 填入',
                  style: AidogType.body.copyWith(color: t.c.fg2),
                ),
              ),
            ),
          ],
        ),
      ],
    );
  }
}
