// 占位外壳：只证明传输层与 i18n 层活着。主题与导航是票 I02 的活，会整个换掉这里。

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

import 'i18n.dart';
import 'transport.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // 文案是构建期资产，不依赖内核 —— 所以「后端连接中」这一屏本身就是翻好的。
  await i18n.init();
  runApp(const AidogI18n(child: AidogApp()));
  unawaitedStart();
}

void unawaitedStart() {
  kernel.start().then((_) => i18n.loadFromBackend()).catchError((Object e) {
    debugPrint('kernel start failed: $e');
  });
}

class AidogApp extends StatelessWidget {
  const AidogApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'aidog',
      theme: ThemeData.dark(useMaterial3: true),
      locale: i18n.flutterLocale,
      supportedLocales: kSupportedFlutterLocales,
      localizationsDelegates: const <LocalizationsDelegate<Object>>[
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      home: const _KernelStatusPage(),
    );
  }
}

class _KernelStatusPage extends StatelessWidget {
  const _KernelStatusPage();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: StreamBuilder<KernelState>(
          stream: kernel.states,
          initialData: kernel.state,
          builder: (context, snap) {
            final state = snap.data ?? KernelState.connecting;
            if (state != KernelState.connected) {
              // 状态名是内部标识，不翻译；也不该被 bidi 重排。
              final label = i18n.t('common.loading');
              final name = ltr(state.name);
              return Text('$label $name');
            }
            return FutureBuilder<Map<String, dynamic>>(
              future: kernel.invoke<Map<String, dynamic>>('about_info'),
              builder: (context, s) {
                if (s.hasError) return Text('about_info failed');
                final version = s.data?['version'] ?? '...';
                final v = ltr(version.toString());
                return Text('aidog $v');
              },
            );
          },
        ),
      ),
    );
  }
}
