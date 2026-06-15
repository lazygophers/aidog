// ─── 应用更新服务 ───────────────────────────────────────────
// 纯前端封装 tauri-plugin-updater (check) + tauri-plugin-process (relaunch)。
// 后端插件/权限/endpoints/pubkey 已就绪 (lib.rs 注册 + capabilities)，此处不碰后端。
//
// 策略：
//   - checkForUpdateDailyThrottled() — 启动调用，localStorage 节流 24h，dev/失败静默。
//   - checkForUpdateManual()         — 手动按钮，忽略节流，区分「已是最新」/失败。
//   - runUpdate(update)              — 下载安装并重启。
// dev / 未签名 / 无网络下 check() 会抛错，节流路径 catch 静默 (console.debug)，不打扰用户。

import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/** localStorage key：上次自动检查更新的时间戳 (ms)。 */
const LAST_CHECK_KEY = "aidog_last_update_check";
/** 自动检查节流间隔：24 小时。 */
const THROTTLE_MS = 24 * 60 * 60 * 1000;

/**
 * 每日节流的后台检查 (应用启动时调用)。
 * - 距上次检查 <24h → 直接返回 null，不触网。
 * - 否则调 check()；无论成功失败都写回当前时间戳 (避免反复检查 / 反复弹窗)。
 * - dev / 未签名 / 无网络抛错 → catch 静默 (console.debug)，返回 null。
 *
 * @returns 有可用更新时返回 Update，否则 null。
 */
export async function checkForUpdateDailyThrottled(): Promise<Update | null> {
  const now = Date.now();
  const raw = localStorage.getItem(LAST_CHECK_KEY);
  const last = raw ? Number(raw) : 0;
  if (Number.isFinite(last) && last > 0 && now - last < THROTTLE_MS) {
    return null;
  }
  // 无论结果如何都写回时间戳，避免失败时每次启动都重试打扰。
  localStorage.setItem(LAST_CHECK_KEY, String(now));
  try {
    return await check();
  } catch (e) {
    // dev / 未签名 / 无网络属预期场景，静默不打扰。
    console.debug("[updater] daily check failed (silent):", e);
    return null;
  }
}

/**
 * 手动检查 (About 页按钮)，忽略节流强制 check。
 * 同样写回时间戳 (让后续自动检查重新计时)。
 * 失败抛错由调用方提示 (与静默的自动检查区分)。
 *
 * @returns 有可用更新时返回 Update，否则 null (已是最新)。
 */
export async function checkForUpdateManual(): Promise<Update | null> {
  localStorage.setItem(LAST_CHECK_KEY, String(Date.now()));
  return await check();
}

/**
 * 下载并安装更新，成功后重启应用。
 * 失败抛错由调用方提示。
 *
 * @param update check() 返回的 Update 对象。
 * @param onProgress 可选下载进度回调 (downloaded/contentLength 字节)。
 */
export async function runUpdate(
  update: Update,
  onProgress?: (downloaded: number, contentLength: number | undefined) => void,
): Promise<void> {
  let downloaded = 0;
  let contentLength: number | undefined;
  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        contentLength = event.data.contentLength;
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        break;
      case "Finished":
        break;
    }
    onProgress?.(downloaded, contentLength);
  });
  await relaunch();
}
