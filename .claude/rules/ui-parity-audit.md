# UI 对齐审计：判「Flutter 缺这个能力」之前必须做的两件事

审 React ↔ Flutter 差异时，下面两类误判各出过一次，都把「已经做好的东西」写成了缺口。下结论前逐条过一遍。

## 1. grep 不到某个 API 名 ≠ 没实现这个能力

Flutter 里同一件事常有两三种写法，只搜一个词会漏。

**踩过的例子**：判「首页四颗 chip 印了 ⌘N/⌘S/⌘L/⌘C 却没有键盘绑定」，依据是 `home.dart` 里 `CallbackShortcuts` / `LogicalKeyboardKey` 零匹配。实际绑定用的是第三种写法 `HardwareKeyboard.instance.addHandler`（`flutter/lib/src/pages/home.dart:79,202-223`），四条口径齐全，还有三条用例守着。

**改法**：按**能力**搜，不按 API 名搜。键盘绑定至少搜 `CallbackShortcuts` / `Shortcuts` / `Actions` / `RawKeyboard` / `HardwareKeyboard` / `KeyboardListener` / `onKeyEvent`；搜完再看一眼这个页面的 `initState` / `dispose`，注册式的监听都挂在那里。都空了再判缺失。

## 2. 树位置 ≠ 渲染位置

`OverlayPortal` / `Overlay` / `showDialog` 的子树画在根 Overlay 上，它在 `children` 列表里排第几与画在哪无关。

**踩过的例子**：判「平台页的 toast 排在页面内容最底部、跟着页面滚」，依据是 `platforms.dart:451` 的 `ToastBar` 是 `Column` 的最后一个 child。实际 `ToastBar` 内部是 `OverlayPortal` + `Positioned(top: 24, left: 0, right: 0)`（`ui_bits.dart:549-557`），画在根 Overlay 的顶部居中，与 React 的 portal 同位置。

**改法**：凡是判定涉及**位置 / 层级 / 会不会跟着滚**，一律看被判 widget 的实现体，确认它有没有 `OverlayPortal` / `Overlay.of` / `showDialog`；判不准就写一条 widget 测试取它的实际 rect，别按下标推。同类 widget：`ToastBar` / `AidogModal` / `ConfirmCard` / `FilterDropdown` 的候选浮层。

## 3. 判定要分三档，不要都塞进「缺失」

- **缺口**：React 有、Flutter 没有，且没人裁过。
- **共同空白**：两侧都没有（例：配额脚本 `requires` 参数两边都不做数字校验）。要改得连 React 一起改，不算 Flutter 缺口。
- **有意偏离**：裁过的（例：关窗不拦未保存改动 —— 用户 2026-09-22 拍板主窗口关闭改成隐藏不销毁，`050212d3`，两侧都故意不弹那个对话框）。

后两档写进表里时必须带上裁决日期与依据，否则下一轮重审会当成缺口再报一次。

## 4. 清单只标注，不重写

重核已有清单时加一列「状态」，值只有 `已修 <commit>` / `仍成立` / `已过期（写明为什么）` 三种。**不删行、不改原判定文字** —— 那是当时的事实，改掉就看不出走过哪条路。判错的那行也留着，把理由写进状态列。
