/// 离页拦截注册表 —— `src/utils/navGuard.ts` 的 Dart 等价物，语义逐条照搬。
///
/// 这里没有 router（和 React 版一样），导航是 [ShellController] 的一个 String 字段。
/// 一个持有未保存改动的页面（如 Claude Code 设置页）在这里注册 guard；导航方调
/// [requestNavigation]：
///   - 没注册 guard  → `proceed()` 立即执行；
///   - 注册了 guard  → 由 guard 决定（例如弹自定义确认框），用户确认后**它自己**调
///                     `proceed()`，取消就把它丢掉。
///
/// 没有这层，编辑中的表单会在点别的导航项时静默丢失。
library;

typedef NavGuard = void Function(void Function() proceed);

NavGuard? _activeGuard;

/// 注册一个离页 guard，返回注销函数。
/// 同一时刻只有一个 guard 生效（后注册者胜）；返回的清理函数只在自己仍是当前 guard 时才清。
void Function() registerNavGuard(NavGuard guard) {
  _activeGuard = guard;
  return () {
    if (identical(_activeGuard, guard)) _activeGuard = null;
  };
}

/// 请求一次导航。有 guard 时由它裁决，否则 [proceed] 同步执行。
void requestNavigation(void Function() proceed) {
  final guard = _activeGuard;
  if (guard != null) {
    guard(proceed);
  } else {
    proceed();
  }
}

/// 仅供测试：清掉残留 guard，避免用例之间互相污染。
void resetNavGuardForTest() => _activeGuard = null;

/// 当前是否有页面声明了脏状态。
bool get hasNavGuard => _activeGuard != null;
