// kernel.ts — 无界面内核管理面设置（票 08）。
//
// 管理面（/rpc/* 全部命令，含改配置、读全部请求日志、执行脚本）**永远只监听 127.0.0.1**，
// 没有开放到局域网的开关。跨机访问由用户自行架反向代理回连本机。
// proxy.ts 里的 `proxyApi.setBindLan` 管的是**转发端口**，与这里毫无关系。

import { invoke } from "../transport";
import type { KernelSettings } from "./types";

export const kernelApi = {
  getSettings: () => invoke<KernelSettings>("kernel_settings_get"),
  setSettings: (settings: KernelSettings) =>
    invoke<void>("kernel_settings_set", { settings }),
};
