// kernel.ts — 管理接口设置（开关 + 可选凭据，ADR 0010）。
// 管理接口挂在代理端口 /api/ 下，只接受本机连接；与 proxy.ts 的 `setBindLan` 无关。

import { invoke } from "../transport";
import type { ApiCallLog, KernelSettings } from "./types";

export const kernelApi = {
  getSettings: () => invoke<KernelSettings>("kernel_settings_get"),
  setSettings: (settings: KernelSettings) =>
    invoke<void>("kernel_settings_set", { settings }),
  /** 最近的管理接口调用记录（只含元数据，保留 7 天）。 */
  callLog: (limit?: number) => invoke<ApiCallLog[]>("api_call_log_list", { limit }),
};
