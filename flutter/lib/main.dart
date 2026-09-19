// 占位外壳：只证明传输层活着。主题与导航是票 I02 的活，会整个换掉这里。

import 'package:flutter/material.dart';

import 'transport.dart';

void main() {
  runApp(const AidogApp());
  unawaitedStart();
}

void unawaitedStart() {
  kernel.start().catchError((Object e) {
    debugPrint('kernel start failed: $e');
    return Uri();
  });
}

class AidogApp extends StatelessWidget {
  const AidogApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'aidog',
      theme: ThemeData.dark(useMaterial3: true),
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
              return Text('aidog-kernel: ${state.name}');
            }
            return FutureBuilder<Map<String, dynamic>>(
              future: kernel.invoke<Map<String, dynamic>>('about_info'),
              builder: (context, s) => Text(
                s.hasError
                    ? 'about_info failed: ${s.error}'
                    : 'aidog ${s.data?['version'] ?? '…'} @ ${kernel.process.address}',
              ),
            );
          },
        ),
      ),
    );
  }
}
