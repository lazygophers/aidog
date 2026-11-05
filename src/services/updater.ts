// ─── 应用更新服务 ───────────────────────────────────────────
// 前端封装 tauri-plugin-updater (check) + tauri-plugin-process (relaunch)。
// 每日自动检查在内核 (aidog_core::update_check)，发现新版本经 updateEvent.ts 回到前端。
//
// 策略：
//   - checkForUpdateManual()         — 手动按钮，忽略节流，区分「已是最新」/失败。
//   - runUpdate(update)              — 下载安装并重启。

import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/**
 * 手动检查 (About 页按钮)，忽略节流强制 check。
 * 失败抛错由调用方提示 (与静默的自动检查区分)。
 *
 * @returns 有可用更新时返回 Update，否则 null (已是最新)。
 */
export async function checkForUpdateManual(): Promise<Update | null> {
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
