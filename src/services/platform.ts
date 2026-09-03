// platform.ts — 桌面原生能力的浏览器兜底（票 10）。
//
// 票 09 的 `transport.ts` 解决的是「命令怎么发」；本模块解决的是「桌面独有的那几件事，
// 在浏览器里怎么办」。同一份 dist 既跑在 Tauri webview 里，也跑在 `aidog-kernel --ui`
// 托管的普通浏览器页里，所以每个能力都按 `isTauri()` 分两条路：
//
// | 能力 | 桌面（Tauri） | 浏览器 |
// |---|---|---|
// | 写剪贴板 | `plugin-clipboard-manager` | `navigator.clipboard` →（非安全上下文）`execCommand` |
// | 读剪贴板 | `plugin-clipboard-manager` | `navigator.clipboard.readText`（要用户授权） |
// | 打开外部链接 | `plugin-opener` | `window.open` |
// | 在文件管理器里定位文件 | `plugin-opener` | **无等价能力** → 退化成「复制路径」 |
// | 应用版本号 | `@tauri-apps/api/app` | 后端 `about_info().app_version` |
// | 系统通知 | `plugin-notification` | `window.Notification` |
// | 本地图片 URL | `convertFileSrc` | 后端 `get_protocol_logo_data_url`（见 useProtocolLogo） |
//
// **静态 import 是安全的**：这些插件模块加载时不碰 `window.__TAURI_INTERNALS__`，
// 只有真调用才会。（会同步抛的是 `getCurrentWebview()` 那类，不在本模块里。）
//
// 剪贴板的非安全上下文兜底不是多余的：内核管理面自己只绑 127.0.0.1（安全上下文），但跨机
// 访问的推荐做法是用户自己架反向代理，反代若以明文 `http://` 对外（局域网里很常见），页面
// 就落在**非**安全上下文里，`navigator.clipboard` 直接是 `undefined`。

import { writeText as tauriWriteText, readText as tauriReadText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl as tauriOpenUrl, revealItemInDir as tauriRevealItemInDir } from "@tauri-apps/plugin-opener";
import { getVersion as tauriGetVersion } from "@tauri-apps/api/app";
import {
  isPermissionGranted as tauriIsPermissionGranted,
  requestPermission as tauriRequestPermission,
  sendNotification as tauriSendNotification,
} from "@tauri-apps/plugin-notification";
import { invoke, isTauri } from "./transport";

export { isTauri };

// ─── 剪贴板 ─────────────────────────────────────────────────

/** 非安全上下文（`http://` + 非 localhost）下 `navigator.clipboard` 不存在时的老办法。 */
function execCommandCopy(text: string): void {
  const ta = document.createElement("textarea");
  ta.value = text;
  // 不能用 display:none —— 选不中就复制不了。挪出视口即可。
  ta.style.position = "fixed";
  ta.style.top = "-9999px";
  document.body.appendChild(ta);
  ta.select();
  try {
    document.execCommand("copy");
  } finally {
    document.body.removeChild(ta);
  }
}

/**
 * 写剪贴板。
 *
 * 桌面走 Tauri 插件（原注释：WKWebView 无手势激活时 `navigator.clipboard` 会静默失败，
 * 插件路径更可靠），浏览器走标准 API。
 */
export async function writeText(text: string): Promise<void> {
  if (isTauri()) return tauriWriteText(text);
  if (navigator.clipboard?.writeText) return navigator.clipboard.writeText(text);
  execCommandCopy(text);
}

/**
 * 读剪贴板。浏览器里需要用户授权（Chrome 会弹权限框），非安全上下文下会抛错，
 * 调用方按原有的失败路径提示即可。
 */
export async function readText(): Promise<string> {
  if (isTauri()) return tauriReadText();
  if (!navigator.clipboard?.readText) {
    throw new Error("clipboard read unavailable (insecure context)");
  }
  return navigator.clipboard.readText();
}

// ─── 打开链接 / 定位文件 ──────────────────────────────────────

/** 用系统默认浏览器打开外部链接；浏览器形态开新标签页。 */
export async function openUrl(url: string): Promise<void> {
  if (isTauri()) return tauriOpenUrl(url);
  window.open(url, "_blank", "noopener,noreferrer");
}

/** 浏览器沙箱里没有「在访达/资源管理器里定位这个文件」这回事，UI 要据此换按钮。 */
export const canRevealItemInDir = (): boolean => isTauri();

/**
 * 在系统文件管理器里定位文件。
 *
 * 浏览器里做不到（沙箱内无本机文件管理器入口），退化成把路径复制到剪贴板 ——
 * 调用方应先用 `canRevealItemInDir()` 换掉按钮文案，别让用户以为窗口会弹出来。
 */
export async function revealItemInDir(path: string): Promise<void> {
  if (isTauri()) return tauriRevealItemInDir(path);
  return writeText(path);
}

// ─── 应用版本 ────────────────────────────────────────────────

/** 应用版本号。浏览器里问后端要（`about_info` 的 `app_version` 就是同一个值）。 */
export async function getAppVersion(): Promise<string> {
  if (isTauri()) return tauriGetVersion();
  const info = await invoke<{ app_version: string }>("about_info");
  return info.app_version;
}

// ─── 系统通知 ────────────────────────────────────────────────

/**
 * 弹一条系统通知（best-effort，失败不抛）。
 *
 * 浏览器侧用 `window.Notification`：需要安全上下文 + 用户授权。拿不到就静默跳过 ——
 * 通知**内容**本身在应用内的通知页里照样看得到，这里只是额外的系统级提醒。
 */
export async function notify(title: string, body: string): Promise<void> {
  try {
    if (isTauri()) {
      let granted = await tauriIsPermissionGranted();
      if (!granted) granted = (await tauriRequestPermission()) === "granted";
      if (granted) tauriSendNotification({ title, body });
      return;
    }
    if (typeof Notification === "undefined") return;
    let perm = Notification.permission;
    if (perm === "default") perm = await Notification.requestPermission();
    if (perm === "granted") new Notification(title, { body });
  } catch (e) {
    console.warn("[platform] notify failed", e);
  }
}
