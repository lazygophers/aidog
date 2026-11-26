// kernelUpdate.ts — 内核更新操作接口（票 magpie-07）。
// 桌面只调内核管理接口（POST /api/rpc/kernel_update_*），不实现第二套下载/校验/替换逻辑：
// 全部流程（清单、验签、原子替换、排空、重启）在 aidog_kernel/src/update.rs。
// 进度不经事件：内核以 kernel_update_status 轮询为唯一进度出口（rpc_download 注释同口径）。

import { invoke, isTauri } from "../transport";
import { proxyApi } from "./proxy";
import { kernelApi } from "./kernel";

/**
 * 桌面形态直连本机代理端口的管理接口：kernel_update_* 路由只装在内核进程上
 * （`aidog_kernel/src/update.rs::install_routes`），桌面壳 IPC 无这组命令，走 invoke 恒失败。
 * 端口 = proxy_get_settings（与 statusline 等消费方同一来源）；auth_token 非空才带 Bearer
 * （空 = 内核不校验，ADR 0010 闸门不放宽：仍只打本机回环）。
 * 浏览器形态（内核 --ui 同源托管）照常走 transport 的 /api/rpc。
 */
async function kernelHttpRpc<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const [{ port }, settings] = await Promise.all([proxyApi.getSettings(), kernelApi.getSettings()]);
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (settings.auth_token) headers.Authorization = `Bearer ${settings.auth_token}`;
  const res = await fetch(`http://127.0.0.1:${port}/api/rpc/${cmd}`, {
    method: "POST",
    headers,
    body: JSON.stringify(args ?? {}),
  });
  // 错误语义与 transport.httpInvoke 对齐：非 JSON body 原文抛出，非 2xx 抛解析后的 JSON。
  const text = await res.text();
  let data: unknown = null;
  if (text.length > 0) {
    try {
      data = JSON.parse(text);
    } catch {
      throw text;
    }
  }
  if (!res.ok) throw data;
  return data as T;
}

function callKernel<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? kernelHttpRpc<T>(cmd, args) : invoke<T>(cmd, args);
}

/** 内核更新阶段（镜像 `aidog_kernel::update::Phase` 的 serde tagged 形状）。 */
export type KernelUpdatePhase =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "downloading"; downloaded: number; total: number | null }
  | { phase: "ready_to_install"; version: string; staged: string }
  | { phase: "installing" }
  | { phase: "failed"; error: string }
  | { phase: "restarting" };

export interface KernelUpdateStatus {
  current_version: string;
  platform_key: string;
  updates_enabled: boolean;
  /** 确认后正在排空的在途请求数。 */
  inflight: number;
  phase: KernelUpdatePhase;
}

export const kernelUpdateApi = {
  status: () => callKernel<KernelUpdateStatus>("kernel_update_status"),
  check: () => callKernel<{ available: boolean; version?: string }>("kernel_update_check"),
  /** 长任务，内核后台执行；进度经 status() 轮询。 */
  download: () => callKernel<{ started: boolean }>("kernel_update_download"),
  install: () => callKernel<{ installed: boolean; version: string }>("kernel_update_install"),
  /** 内核先应答再排空退出；连接可能随后掐断，调用方按 status 轮询确认。 */
  confirm: () => callKernel<{ restarting: boolean }>("kernel_update_confirm"),
};
