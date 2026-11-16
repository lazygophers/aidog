import type { KeyboardEvent } from "react";

// 输入法组字时的确认键 Enter 不应触发提交。WKWebView/Safari 的确认键到达时 isComposing 已复位，
// 只剩 keyCode 229，所以两者都要查（指南 ime-safe-enter-submit）。
export const isImeComposing = (e: KeyboardEvent) => e.nativeEvent.isComposing || e.keyCode === 229;
