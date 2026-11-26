// ─── 应用更新服务 ───────────────────────────────────────────
// 前端封装 tauri-plugin-updater (check) + tauri-plugin-process (relaunch)。
// 每日自动检查在内核 (aidog_core::update_check)，发现新版本经 updateEvent.ts 回到前端。
//
// 策略：
//   - checkForUpdateManual()         — 手动按钮，忽略节流，区分「已是最新」/失败。
//   - downloadUpdate(update)         — 下载并验签，回传进度。
//   - installUpdate(update)          — 安装（包已验签）。
//   - beginDrain() / relaunchWhenDrained() — 确认重启后拒新请求、等在途清零再重启。

import { check, type Update } from "@tauri-apps/plugin-updater";
import { invoke } from "@tauri-apps/api/core";

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
 * 下载并验签；成功后保留已下载包，等待调用方确认安装。
 * 失败抛错由调用方提示。
 *
 * @param update check() 返回的 Update 对象。
 * @param onProgress 可选下载进度回调 (downloaded/contentLength 字节)。
 */
export async function downloadUpdate(
  update: Update,
  onProgress?: (downloaded: number, contentLength: number | undefined) => void,
): Promise<void> {
  let downloaded = 0;
  let contentLength: number | undefined;
  await update.download((event) => {
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
}

/** 安装；只能在 downloadUpdate() 成功返回后调用（下载已通过签名校验）。 */
export async function installUpdate(update: Update): Promise<void> {
  await update.install();
}

/** 置内核排空态：代理拒收新请求（503 updating）；返回 false 表示已在排空中。 */
export async function beginDrain(): Promise<boolean> {
  return await invoke<boolean>("update_begin_drain");
}

/** 解除排空态：安装失败或取消更新后恢复代理正常服务；返回 false 表示本就不在排空中。 */
export async function abortDrain(): Promise<boolean> {
  return await invoke<boolean>("update_abort_drain");
}

/** 在途请求清零后由内核侧重启进新版；应用会退出，无返回路径。 */
export async function relaunchWhenDrained(): Promise<void> {
  await invoke("update_relaunch_when_drained");
}
