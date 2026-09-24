/// `aidog://` 深链的 Dart 侧协议层 + 页面消费总线（补核 A）。
///
/// 对应 React 那边的两段：Rust `src-tauri/src/deep_link.rs`（URL →
/// `{entity, action, data}`）+ `src/App.tsx:123-143` 的「缓存 + CustomEvent」
/// 二次分发。Flutter 壳里 URL 由 app_links 抛进 `uriLinkStream`，
/// `main.dart` 解析后送进本总线；目标页两条路径消费（照 React 同构）：
/// ① mount 时取一次缓存（冷启动 / 在别页时被唤起，切过来才挂载）；
/// ② 运行中收事件（本页已挂载的热路径）。
///
/// URL 格式：`aidog://<entity>/<action>?data=<base64>`
///（entity 落在 host 段，action 取 path 首段缺省 `import`，data 取首个
/// `data=` query 缺省空串 —— 与 `deep_link.rs::parse_url` 同一语义）。
library;

import 'dart:async';

/// 解析结果，形同 `deep_link.rs:29::DeepLinkPayload`。
typedef DeepLinkPayload = ({String entity, String action, String data});

/// 解析单个 URL。scheme 非 `aidog` / 缺 entity → null（调用方 warn 跳过）。
///
/// 差异记录：Dart `Uri.queryParameters` 同名字段取**末值**，Rust
/// `query_pairs` 取首值 —— base64 无歧义、深链不构造重复 query，实际不达。
DeepLinkPayload? parseDeepLink(Uri uri) {
  if (uri.scheme != 'aidog') return null;
  final entity = uri.host;
  if (entity.isEmpty) return null;
  final segments = uri.pathSegments;
  final action = segments.isEmpty || segments.first.isEmpty
      ? 'import'
      : segments.first;
  return (
    entity: entity,
    action: action,
    data: uri.queryParameters['data'] ?? '',
  );
}

/// 深链分发总线：per-entity 缓存 + 广播事件。
///
/// 照 React：dispatch 同时写 `window.__aidogDeepLink[entity]`（mount 消费）
/// 和发 `aidog:<entity>` 事件（热路径消费），两条消费路径都删缓存防重放。
/// ponytail: per-entity 单条缓存（非队列），last-write-wins，深链不要求保序。
class DeepLinkBus {
  final Map<String, DeepLinkPayload> _pending = <String, DeepLinkPayload>{};
  final StreamController<DeepLinkPayload> _events =
      StreamController<DeepLinkPayload>.broadcast(sync: true);

  /// 生产端（`main.dart`）：解析成功后调。缓存 + 广播两路同时走。
  void dispatch(DeepLinkPayload payload) {
    _pending[payload.entity] = payload;
    _events.add(payload);
  }

  /// 消费路径 ①：页面 mount 时取一次，取走即删（防重复导入）。
  DeepLinkPayload? takePending(String entity) => _pending.remove(entity);

  /// 消费路径 ②：只收本 entity 的事件；事件到达时也删缓存
  ///（React 的 mount / 事件两路都 `delete w.__aidogDeepLink`）。
  StreamSubscription<DeepLinkPayload> subscribe(
    String entity,
    void Function(DeepLinkPayload payload) onEvent,
  ) => _events.stream.where((p) => p.entity == entity).listen((p) {
    _pending.remove(entity);
    onEvent(p);
  });

  void dispose() => _events.close();
}

/// 应用级单例：`main.dart` 是生产端，Skills / MCP 页是消费端。
final DeepLinkBus deepLinks = DeepLinkBus();
