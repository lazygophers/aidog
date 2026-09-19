// 票 I12：六项原生能力的回归测试。
//
// 对照组是 React 版的 `src/services/platform.test.ts` —— 那边每条能力证明「桌面走插件、
// 浏览器走 Web API」，这边证明「走对了 Flutter 包、降级路径与 platform.ts 一字不差」。
//
// **哪些是真跑、哪些不是，写明白**：
//   - 进程（`spawnDetached`）：**全真**。真起一个脱离本进程的子进程，看它真写了文件。
//   - 剪贴板：**真**走 SDK 的 `Clipboard` → `SystemChannels.platform`，只有 OS 粘贴板那一端
//     被测试用的 binary messenger 接住（flutter_test 里没有真粘贴板，这是唯一的边界）。
//   - 通知：**真**走 `FlutterLocalNotificationsPlugin` → `MacOSFlutterLocalNotificationsPlugin`
//     → 真实 MethodChannel（`dexterous.com/flutter/local_notifications`），只有 ObjC 那端是假的。
//   - 定位文件：命令拼法 + 降级分支**真跑**（降级真写了剪贴板）；`open -R` 不在自动化里真执行
//     ——它会抢焦点弹访达。改为断言那个可执行文件真的存在。
//   - 文件对话框 / 打开链接：走两个包**自己的** platform-interface 测试接缝。原生面板本身
//     没法在无人值守的测试里跑（文件对话框会一直等人点）。
//   - 版本号 + 内核弹窗桥：在 `platform_integration_test.dart` 里对着**真内核**跑。

import 'dart:io';

import 'package:aidog_flutter/platform.dart';
import 'package:file_selector_platform_interface/file_selector_platform_interface.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:plugin_platform_interface/plugin_platform_interface.dart';
import 'package:url_launcher_platform_interface/link.dart';
import 'package:url_launcher_platform_interface/url_launcher_platform_interface.dart';

/// 录下 `file_selector` 收到的参数。用包自己的 platform interface 接缝 —— 它的官方测试
/// 就是这么做的，被测的是**我们这层的参数映射**，不是把自己 mock 成同义反复。
class _RecordingFileSelector extends FileSelectorPlatform {
  List<XTypeGroup>? groups;
  String? initialDirectory;
  String? suggestedName;
  String? confirmButtonText;
  String? lastCall;

  String? fileResult;
  String? dirResult;
  String? saveResult;

  @override
  Future<XFile?> openFile({
    List<XTypeGroup>? acceptedTypeGroups,
    String? initialDirectory,
    String? confirmButtonText,
  }) async {
    lastCall = 'openFile';
    groups = acceptedTypeGroups;
    this.initialDirectory = initialDirectory;
    this.confirmButtonText = confirmButtonText;
    return fileResult == null ? null : XFile(fileResult!);
  }

  @override
  Future<String?> getDirectoryPath({
    String? initialDirectory,
    String? confirmButtonText,
  }) async {
    lastCall = 'getDirectoryPath';
    this.initialDirectory = initialDirectory;
    this.confirmButtonText = confirmButtonText;
    return dirResult;
  }

  @override
  Future<FileSaveLocation?> getSaveLocation({
    List<XTypeGroup>? acceptedTypeGroups,
    SaveDialogOptions options = const SaveDialogOptions(),
  }) async {
    lastCall = 'getSaveLocation';
    groups = acceptedTypeGroups;
    initialDirectory = options.initialDirectory;
    suggestedName = options.suggestedName;
    confirmButtonText = options.confirmButtonText;
    return saveResult == null ? null : FileSaveLocation(saveResult!);
  }
}

class _RecordingLauncher extends UrlLauncherPlatform
    with MockPlatformInterfaceMixin {
  String? launchedUrl;
  PreferredLaunchMode? mode;
  bool result = true;

  @override
  LinkDelegate? get linkDelegate => null;

  @override
  Future<bool> launchUrl(String url, LaunchOptions options) async {
    launchedUrl = url;
    mode = options.mode;
    return result;
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('剪贴板', () {
    test('写 / 读走 SDK 的 Clipboard（真实 SystemChannels.platform 往返）', () async {
      String? written;
      TestDefaultBinaryMessengerBinding
          .instance
          .defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, (call) async {
            if (call.method == 'Clipboard.setData') {
              written = (call.arguments as Map)['text'] as String;
              return null;
            }
            if (call.method == 'Clipboard.getData') {
              return <String, Object?>{'text': 'from-clipboard'};
            }
            return null;
          });
      addTearDown(
        () => TestDefaultBinaryMessengerBinding
            .instance
            .defaultBinaryMessenger
            .setMockMethodCallHandler(SystemChannels.platform, null),
      );

      await writeText('hi');
      expect(written, 'hi');
      expect(await readText(), 'from-clipboard');
    });

    test('剪贴板里没有文本时返回空串，不抛（桌面壳没有「非安全上下文」这种形态）', () async {
      TestDefaultBinaryMessengerBinding
          .instance
          .defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, (call) async => null);
      addTearDown(
        () => TestDefaultBinaryMessengerBinding
            .instance
            .defaultBinaryMessenger
            .setMockMethodCallHandler(SystemChannels.platform, null),
      );
      expect(await readText(), '');
    });
  });

  group('定位文件', () {
    test('macOS 是 `open -R`，Windows 是 `explorer /select,<path>`（逗号后无空格）', () {
      expect(revealCommand('/tmp/backup.aidogx', os: 'macos'), <String>[
        'open',
        '-R',
        '/tmp/backup.aidogx',
      ]);
      expect(revealCommand(r'C:\tmp\backup.aidogx', os: 'windows'), <String>[
        'explorer',
        r'/select,C:\tmp\backup.aidogx',
      ]);
      expect(revealCommand('/tmp/x', os: 'linux'), isNull);
    });

    test('macOS 上那个可执行文件真的存在（命令拼对了但程序不在，等于白拼）', () {
      final which = Process.runSync('/usr/bin/which', <String>['open']);
      expect(
        which.exitCode,
        0,
        reason: 'macOS 上 `open` 必须在 PATH 里；本条只在 macOS 有意义',
      );
    }, skip: !Platform.isMacOS);

    test('没有定位能力的 OS 上退化成复制路径（照抄 platform.ts:95-98）', () async {
      String? written;
      TestDefaultBinaryMessengerBinding
          .instance
          .defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, (call) async {
            if (call.method == 'Clipboard.setData') {
              written = (call.arguments as Map)['text'] as String;
            }
            return null;
          });
      addTearDown(
        () => TestDefaultBinaryMessengerBinding
            .instance
            .defaultBinaryMessenger
            .setMockMethodCallHandler(SystemChannels.platform, null),
      );

      await revealItemInDir('/tmp/backup.aidogx', os: 'linux');
      expect(
        written,
        '/tmp/backup.aidogx',
        reason: '降级不是「什么都不做」，是把路径给用户',
      );
    });

    test('canRevealItemInDir 与当前 OS 一致（UI 据此换按钮文案）', () {
      expect(canRevealItemInDir(), Platform.isMacOS || Platform.isWindows);
    });
  });

  group('文件对话框', () {
    late _RecordingFileSelector fake;

    setUp(() {
      fake = _RecordingFileSelector();
      FileSelectorPlatform.instance = fake;
    });

    test('选文件：filters → XTypeGroup，defaultPath → initialDirectory', () async {
      fake.fileResult = '/tmp/a.json';
      final picked = await pickPath(
        const PickPathOptions(
          defaultPath: '/tmp',
          title: '选一个',
          filters: <PickFilter>[
            PickFilter('备份', <String>['aidogx']),
            PickFilter('JSON', <String>['json']),
          ],
        ),
      );
      expect(picked, '/tmp/a.json');
      expect(fake.lastCall, 'openFile');
      expect(fake.initialDirectory, '/tmp');
      expect(fake.confirmButtonText, '选一个');
      expect(fake.groups!.map((g) => g.label), <String>['备份', 'JSON']);
      expect(fake.groups!.first.extensions, <String>['aidogx']);
    });

    test('选目录：走 getDirectoryPath，不是 openFile', () async {
      fake.dirResult = '/tmp/dir';
      expect(
        await pickPath(const PickPathOptions(directory: true, defaultPath: '/tmp')),
        '/tmp/dir',
      );
      expect(fake.lastCall, 'getDirectoryPath');
      expect(fake.initialDirectory, '/tmp');
    });

    test('保存：defaultPath 当预填文件名（对齐 Tauri save 的 defaultPath 语义）', () async {
      fake.saveResult = '/tmp/out.aidogx';
      expect(
        await pickPath(
          const PickPathOptions(save: true, defaultPath: 'out.aidogx'),
        ),
        '/tmp/out.aidogx',
      );
      expect(fake.lastCall, 'getSaveLocation');
      expect(fake.suggestedName, 'out.aidogx');
    });

    test('save 与 directory 同时给：save 赢（与 pathPicker.ts:57 的分支顺序一致）', () async {
      fake.saveResult = '/tmp/out';
      await pickPath(const PickPathOptions(save: true, directory: true));
      expect(fake.lastCall, 'getSaveLocation');
    });

    test('用户取消 → null（三种模式都是）', () async {
      expect(await pickPath(), isNull);
      expect(await pickPath(const PickPathOptions(directory: true)), isNull);
      expect(await pickPath(const PickPathOptions(save: true)), isNull);
    });
  });

  group('打开链接', () {
    test('用外部应用打开（不是应用内 webview）', () async {
      final fake = _RecordingLauncher();
      UrlLauncherPlatform.instance = fake;
      await openUrl('https://aidog.dev/docs');
      expect(fake.launchedUrl, 'https://aidog.dev/docs');
      expect(
        fake.mode,
        PreferredLaunchMode.externalApplication,
        reason: 'Tauri 的 openUrl 是交给系统默认浏览器，不是内嵌打开',
      );
    });

    test('打不开就抛，不静默吞（「点了没反应」查不出原因）', () async {
      final fake = _RecordingLauncher()..result = false;
      UrlLauncherPlatform.instance = fake;
      await expectLater(openUrl('nope://x'), throwsStateError);
    });
  });

  group('进程 / 重启', () {
    test('spawnDetached 真起一个脱离本进程的子进程，它真跑完了', () async {
      final dir = Directory.systemTemp.createTempSync('aidog-I12-proc');
      addTearDown(() => dir.deleteSync(recursive: true));
      final marker = '${dir.path}/ran';

      final pid = await spawnDetached(<String>[
        '/bin/sh',
        '-c',
        'echo relaunched > $marker',
      ]);
      expect(pid, greaterThan(0));

      final deadline = DateTime.now().add(const Duration(seconds: 10));
      while (!File(marker).existsSync() && DateTime.now().isBefore(deadline)) {
        await Future<void>.delayed(const Duration(milliseconds: 50));
      }
      expect(
        File(marker).existsSync(),
        isTrue,
        reason: '10 秒内子进程没跑起来 —— detached 起不来就没法重启应用',
      );
      expect(File(marker).readAsStringSync().trim(), 'relaunched');
    }, skip: Platform.isWindows);

    test('重启命令 = 当前可执行文件 + 原样的启动参数', () {
      final cmd = relaunchCommand();
      expect(cmd.first, Platform.resolvedExecutable);
      expect(File(cmd.first).existsSync(), isTrue);
      expect(cmd.sublist(1), Platform.executableArguments);
    });
  });

  group('系统通知', () {
    late List<MethodCall> calls;

    setUp(() {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      // 插件自己的注册入口：把 MacOSFlutterLocalNotificationsPlugin 装成当前实现。
      // 桌面上跑时由 flutter 的 registrant 自动调，测试里手动调一次。
      MacOSFlutterLocalNotificationsPlugin.registerWith();
      calls = <MethodCall>[];
      TestDefaultBinaryMessengerBinding
          .instance
          .defaultBinaryMessenger
          .setMockMethodCallHandler(
            const MethodChannel('dexterous.com/flutter/local_notifications'),
            (call) async {
              calls.add(call);
              return call.method == 'initialize' ? true : null;
            },
          );
    });

    tearDown(() {
      debugDefaultTargetPlatformOverride = null;
      TestDefaultBinaryMessengerBinding
          .instance
          .defaultBinaryMessenger
          .setMockMethodCallHandler(
            const MethodChannel('dexterous.com/flutter/local_notifications'),
            null,
          );
    });

    test('弹一条：先 initialize（含权限申请）再 show，title/body 原样落到插件上', () async {
      await notify('AiDog', '代理启动失败：端口 9890 被占用');

      expect(calls.map((c) => c.method), containsAllInOrder(<String>['initialize', 'show']));
      final init = calls.firstWhere((c) => c.method == 'initialize');
      expect(
        (init.arguments as Map)['requestAlertPermission'],
        isTrue,
        reason: '对齐 Rust 桌面壳启动时的 request_permission（app_setup.rs:406）',
      );
      final show = calls.firstWhere((c) => c.method == 'show');
      expect((show.arguments as Map)['title'], 'AiDog');
      expect((show.arguments as Map)['body'], '代理启动失败：端口 9890 被占用');
    });

    test('initialize 只发一次（幂等），但每条通知 id 不同（后一条不覆盖前一条）', () async {
      calls.clear();
      await notify('a', '1');
      await notify('b', '2');
      expect(calls.where((c) => c.method == 'initialize'), hasLength(0),
          reason: '上一条测试已经初始化过了，_initialized 应当记住');
      final ids = calls
          .where((c) => c.method == 'show')
          .map((c) => (c.arguments as Map)['id'])
          .toList();
      expect(ids, hasLength(2));
      expect(ids.first, isNot(ids.last));
    });

    test('通道抛错时静默跳过，不把调用方拖崩（对照 platform.test.ts:148）', () async {
      TestDefaultBinaryMessengerBinding
          .instance
          .defaultBinaryMessenger
          .setMockMethodCallHandler(
            const MethodChannel('dexterous.com/flutter/local_notifications'),
            (call) async => throw PlatformException(code: 'no-permission'),
          );
      await expectLater(notify('t', 'b'), completes);
    });

    test('定时 / 重复通知的缺口：本项目一条都没用到，所以现状零影响', () {
      // 缺口本身是真的（macOS 无 schedule/showDailyAtTime/showWeeklyAtDayAndTime、
      // Linux 无调度 API、Windows periodicallyShow* 抛 UnsupportedError），
      // 但本层只暴露 `notify`（立刻弹），压根没有调度入口 —— 没有 API 可以被缺口影响。
      expect(schedulingGap(), isNotNull, reason: 'macOS 上确实有缺口，不粉饰');
      // 真正的断言：这一层没有任何定时 / 重复通知的入口。
      final api = File('lib/platform.dart').readAsStringSync();
      for (final gap in <String>[
        'zonedSchedule',
        'periodicallyShow',
        'showDailyAtTime',
        'showWeeklyAtDayAndTime',
      ]) {
        expect(
          RegExp('$gap\\s*\\(').hasMatch(api),
          isFalse,
          reason: '$gap 一旦被用上，三平台缺口就从「不相干」变成「真缺功能」，'
              '这条断言就是那时候的报警器',
        );
      }
    });
  });

  test('事件名与 Rust 侧常量一致（aidog_notification::NOTIF_POPUP）', () {
    expect(kNotifPopup, 'notif-popup');
  });
}
