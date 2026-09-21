/// 菜单栏（macOS `NSStatusItem`）的 Dart 侧（票 I20）。
///
/// 图标、文字、右键菜单全在原生那边：`macos/Runner/MenuBar.swift` 把 Rust 的
/// `aidog_core::menubar` 链进了 Runner，排版用的是和 Tauri 壳同一份
/// `tray_render`（单一实现，不在 Dart 里重写）。这一层只干两件事：
///
/// 1. **喂数**：从内核取 `menu_bar_state`，整块 JSON 原样推给 Swift。
/// 2. **接菜单动作**：「启停代理」要打内核 RPC，原生那边没有连接，所以回到这里来做。
///    「显示主窗口」「退出」是纯原生动作，不经过 Dart。
///
/// 刷新时机与 Tauri 壳一致：后端 emit `tray-refresh`（代理启停 / 配额刷新 / 请求落库
/// 都会发）→ 防抖 1 秒 → 重取一帧。1 秒是对齐托盘设置页的 `TrayConfigTab.tsx:208`。
///
/// 非 macOS 上整层是 no-op（[menuBarSupported]）—— Windows 的托盘没有标题文字这回事。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io' show Platform;

import 'package:flutter/services.dart';

import '../transport.dart';
import 'pages/invoke.dart';

/// 原生菜单栏宿主的通道。Dart → Swift 只有 `render`；Swift → Dart 只有 `toggleProxy`。
const MethodChannel kMenuBarChannel = MethodChannel('aidog/menubar');

/// 后端「托盘该刷新了」事件名。与 Tauri 壳 `app_setup.rs` 监听的是同一个。
const String kTrayRefreshEvent = 'tray-refresh';

/// 只有 macOS 有带标题的菜单栏条目。
bool get menuBarSupported => Platform.isMacOS;

/// 菜单栏的喂数循环。进程级单例，`main()` 里起一次。
class MenuBarBridge {
  MenuBarBridge({
    InvokeFn? invoke,
    this.channel = kMenuBarChannel,
    Stream<Object?> Function()? refreshEvents,
    this.debounce = const Duration(seconds: 1),
  }) : _invoke = invoke ?? kernelInvoke,
       _refreshEvents =
           refreshEvents ?? (() => kernel.on<Object?>(kTrayRefreshEvent));

  final InvokeFn _invoke;

  /// 原生宿主的通道。测试里换成假通道。
  final MethodChannel channel;
  final Stream<Object?> Function() _refreshEvents;

  /// 刷新防抖窗口。对齐托盘设置页的 1000 ms（`TrayConfigTab.tsx:208`）。
  final Duration debounce;

  StreamSubscription<void>? _sub;

  /// 画第一帧并开始跟着 `tray-refresh` 刷新。重复调用无副作用。
  Future<void> start() async {
    if (_sub != null) return;
    channel.setMethodCallHandler(_onNativeCall);
    _sub = debounceStream(
      _refreshEvents(),
      delay: debounce,
    ).listen((_) => unawaited(refresh()));
    await refresh();
  }

  /// 取一帧喂给原生。内核还没连上 / 命令失败时保持上一帧 —— 菜单栏留着旧数字，
  /// 比闪成空白强；下一个 `tray-refresh` 会自己追上。
  Future<void> refresh() async {
    final Object? state;
    try {
      state = await _invoke('menu_bar_state');
    } catch (_) {
      return;
    }
    await channel.invokeMethod<void>('render', jsonEncode(state));
  }

  /// 菜单里的「Start / Stop Proxy」。[running] = 点击时的状态，true 意味着该停。
  ///
  /// 端口取用户设置里的那个，与设置页的启动按钮同源（`system_logic.dart:233`+`:299`）。
  /// 启停完成后后端自己会 emit `tray-refresh`，菜单项的字面随下一帧更新。
  Future<void> _toggleProxy(bool running) async {
    if (running) {
      await _invoke('proxy_stop');
      return;
    }
    final settings = await _invoke('proxy_get_settings');
    final port = settings is Map ? (settings['port'] as num?)?.toInt() : null;
    await _invoke('proxy_start', {'port': port ?? 9890});
  }

  Future<Object?> _onNativeCall(MethodCall call) async {
    switch (call.method) {
      case 'toggleProxy':
        await _toggleProxy(call.arguments == true);
        return null;
      default:
        throw MissingPluginException('menubar: ${call.method}');
    }
  }

  Future<void> dispose() async {
    await _sub?.cancel();
    _sub = null;
    channel.setMethodCallHandler(null);
  }
}

/// 进程级单例。测试里自己 new 一个，别用它。
final MenuBarBridge menuBar = MenuBarBridge();
