// pathPicker.ts — 本地路径选择的双形态实现（票 10）。
//
// 桌面版弹**原生文件对话框**（`@tauri-apps/plugin-dialog`），行为与改造前一字不差。
// 浏览器里没有这东西——沙箱内的 `<input type=file>` 只给文件内容，**拿不到绝对路径**，
// 而 aidog 这几处要的正是路径（后端按路径读写）。所以浏览器形态退化成
// **文本框 + 服务端路径补全**（后端 `fs_autocomplete` 命令，本就存在）。
//
// 这是形态本身的约束，不是实现取舍：浏览器里没有第二条路。
//
// 用法（6 处调用点全一样）：
//
//     const picked = await pickPath({ directory: true, title: "..." });
//     if (!picked) return;   // 用户取消
//
// 浏览器分支靠一个模块级订阅者把请求推给挂在 App 根上的 `<PathPickerHost/>`，
// 它渲染弹窗、拿到结果后 resolve 上面这个 Promise。

import { open as tauriOpen, save as tauriSave } from "@tauri-apps/plugin-dialog";
import { isTauri } from "./transport";

export interface PickPathOptions {
  /** 选目录（默认选文件）。与 `save` 互斥。 */
  directory?: boolean;
  /** 选「保存到哪」而非「打开哪个」——桌面走原生保存对话框。 */
  save?: boolean;
  /** 保存模式的预填文件名 / 打开模式的预填路径。 */
  defaultPath?: string;
  /** 对话框标题，也是浏览器弹窗的标题。 */
  title?: string;
  /** 扩展名过滤（桌面原生对话框用；浏览器形态只作提示文案）。 */
  filters?: { name: string; extensions: string[] }[];
}

/** 浏览器弹窗的一次请求。`resolve` 由 `<PathPickerHost/>` 调用。 */
export interface PathPickerRequest {
  options: PickPathOptions;
  resolve: (path: string | null) => void;
}

type Subscriber = (req: PathPickerRequest) => void;

let subscriber: Subscriber | null = null;

/** 由 `<PathPickerHost/>` 在 mount 时登记，unmount 时注销。生产代码勿直接调。 */
export function registerPathPickerHost(fn: Subscriber | null): void {
  subscriber = fn;
}

/**
 * 选一个本地路径。返回绝对路径；用户取消返回 `null`。
 *
 * - 桌面（Tauri）：原生文件 / 目录 / 保存对话框。
 * - 浏览器：文本框 + 服务端补全的弹窗。
 */
export async function pickPath(options: PickPathOptions = {}): Promise<string | null> {
  if (isTauri()) {
    if (options.save) {
      const picked = await tauriSave({
        defaultPath: options.defaultPath,
        filters: options.filters,
        title: options.title,
      });
      return picked ?? null;
    }
    const picked = await tauriOpen({
      directory: options.directory,
      multiple: false,
      defaultPath: options.defaultPath,
      filters: options.filters,
      title: options.title,
    });
    return typeof picked === "string" ? picked : null;
  }

  if (!subscriber) {
    // Host 没挂上（例如悬浮窗那个 entry）——不静默假装取消，把原因说出来。
    throw new Error("path picker host not mounted");
  }
  const host = subscriber;
  return new Promise<string | null>((resolve) => {
    host({ options, resolve });
  });
}
