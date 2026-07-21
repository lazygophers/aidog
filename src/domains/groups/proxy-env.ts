import { useEffect, useState } from "react";
import { settingsApi } from "../../services/api";
import type { EnvVar } from "../../services/api";

/**
 * 出站代理 env 键：写入 claude settings.json env 段（Coding 设置代理卡片），
 * 同时在「复制 Codex 启动命令」时前置 export 注入 codex 进程环境。
 * Codex config.toml 无原生 proxy 字段（官方 issue #4242/#6060 未实现），出站代理靠 process env。
 */
export const PROXY_ENV_KEYS = ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY"] as const;

/**
 * 从 claude settings.json env 段读 4 个代理键（非空）→ EnvVar[]。
 * 读失败或键缺失返空数组（不抛错，调用方 fallback 到无代理命令）。
 */
export async function loadProxyEnvVars(): Promise<EnvVar[]> {
  try {
    const cfg = (await settingsApi.get("global", "claude_code")) ?? {};
    const env = (cfg as Record<string, any> | null)?.env as Record<string, any> | undefined;
    if (!env) return [];
    return PROXY_ENV_KEYS
      .filter((k) => typeof env[k] === "string" && env[k] !== "")
      .map((k) => ({ key: k, value: String(env[k]) }));
  } catch {
    return [];
  }
}

/**
 * mount 时加载一次代理 EnvVar（供 buildCodexCommand 合并）。
 * 读失败静默返空。组件 unmount 后的 resolve 丢弃（防 setState on unmounted）。
 */
export function useProxyEnvVars(): EnvVar[] {
  const [vars, setVars] = useState<EnvVar[]>([]);
  useEffect(() => {
    let cancelled = false;
    loadProxyEnvVars().then((v) => { if (!cancelled) setVars(v); }).catch(() => {});
    return () => { cancelled = true; };
  }, []);
  return vars;
}
