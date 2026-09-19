/// 主窗口入口。骨架、主题、导航全在 `shell.dart`；页面内容是票 I06-I09 的活，
/// 这里先按 activeId 占位，页面票逐个替换 `_placeholder`。
library;

import 'package:flutter/material.dart';

import 'shell.dart';
import 'transport.dart';

void main() {
  runApp(const AidogApp());
  kernel.start().catchError((Object e) {
    debugPrint('kernel start failed: $e');
    return Uri();
  });
}

/// 临时取词：票 I03 会接 8 语言的真 locale 表，到时整块删掉。
/// 只覆盖骨架自己要显示的 key，页面文案一律走 I03。
const Map<String, String> _stubZhHans = {
  'nav.section.overview': '概览',
  'nav.section.platform': '平台',
  'nav.section.logStats': '日志统计',
  'nav.section.extension': '扩展',
  'nav.section.system': '系统',
  'nav.home': '首页',
  'nav.platforms': 'AI 平台',
  'nav.stats': '使用统计',
  'nav.logs': '代理日志',
  'nav.requestLog': '请求日志',
  'nav.notifications': '通知中心',
  'nav.skills': 'Skills',
  'nav.mcp': 'MCP',
  'nav.settings': '设置',
  'nav.about': '关于',
  'nav.collapse': '折叠侧栏',
  'nav.settingsGroup.general': '常规',
  'nav.settingsGroup.integration': '集成',
  'nav.settingsGroup.rules': '规则',
  'nav.settingsGroup.notification': '通知',
  'nav.settingsGroup.config': '配置',
  'appSettings.systemTab': '系统',
  'appSettings.cliIntegrationTab': 'Coding 设置',
  'appSettings.claudeTab': 'Claude',
  'appSettings.codexTab': 'Codex',
  'appSettings.piTab': 'pi',
  'appSettings.middlewareTab': '中间件',
  'appSettings.schedulingTab': '调度熔断',
  'appSettings.notificationsTab': '系统通知',
  'appSettings.modelInfoTab': '模型信息',
  'appSettings.trayTab': '托盘',
  'appSettings.popoverTab': '浮窗',
  'appSettings.importExportTab': '导入导出',
  'appSettings.mitmTab': 'MITM 解密',
  'theme.dark': '深色',
  'theme.light': '浅色',
};

String _t(String key) => _stubZhHans[key] ?? key;

class AidogApp extends StatefulWidget {
  const AidogApp({super.key});

  @override
  State<AidogApp> createState() => _AidogAppState();
}

class _AidogAppState extends State<AidogApp> {
  final _nav = ShellController();
  final _theme = ThemeController();

  @override
  void dispose() {
    _nav.dispose();
    _theme.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return StreamBuilder<KernelState>(
      stream: kernel.states,
      initialData: kernel.state,
      builder: (context, snap) {
        final connected = snap.data == KernelState.connected;
        return AidogShellApp(
          controller: _nav,
          theme: _theme,
          t: _t,
          localeLabel: '简体中文',
          live: connected,
          status: connected
              ? '${kernel.process.address?.host}:${kernel.process.address?.port}'
              : (snap.data ?? KernelState.connecting).name,
          pageBuilder: (context, id) => _placeholder(context, id),
        );
      },
    );
  }

  Widget _placeholder(BuildContext context, String id) {
    final t = AidogTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHead(title: _t('nav.${id.split('/').first}'), subtitle: id),
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
