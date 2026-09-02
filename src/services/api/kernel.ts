// kernel.ts — 无界面内核管理面设置（票 08）。
//
// 这里的 bind_lan 管的是**管理接口**（/rpc/* 共 210 个命令，含改配置、读全部请求日志、
// 执行脚本），与 proxy.ts 里 `proxyApi.setBindLan` 管的**转发端口**是两个互不相干的开关。
// 两边在设置页也分列两处，别在任何一侧读另一侧的值。

import { invoke } from "../transport";
import type { KernelSettings } from "./types";

// 开启绑定开关但未配鉴权凭据时，后端 reject 的字符串就是这个 i18n key。
export const KERNEL_BIND_REQUIRES_AUTH = "kernel.bindLanRequiresAuth";

export const kernelApi = {
  getSettings: () => invoke<KernelSettings>("kernel_settings_get"),
  setSettings: (settings: KernelSettings) =>
    invoke<void>("kernel_settings_set", { settings }),
  // 未配凭据时开启会 reject（消息为 KERNEL_BIND_REQUIRES_AUTH），DB 里的值保持原样。
  setBindLan: (enabled: boolean) =>
    invoke<void>("kernel_set_bind_lan", { enabled }),
};
