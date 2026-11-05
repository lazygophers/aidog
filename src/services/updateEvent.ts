// 内核每日检查发现新版本 → emit `aidog-update-available`；这里取 Update 对象交给现有更新提示。
// 返回 Promise 供调用方等监听挂好再通知内核「界面就绪」（事件不排队）。
import { check, type Update } from "@tauri-apps/plugin-updater";
import { listen } from "./transport";

export function listenKernelUpdate(onUpdate: (u: Update) => void): Promise<() => void> {
  return listen("aidog-update-available", async () => {
    try {
      const u = await check();
      if (u) onUpdate(u);
    } catch (e) {
      console.debug("[updater] kernel-notified check failed (silent):", e);
    }
  });
}
