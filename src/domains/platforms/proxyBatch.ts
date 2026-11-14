// proxyBatch — 平台批量代理写入（review 判断项 3：从 proxyConfig.ts 拆出，职责单一）。
// 消费者：系统设置页「为全部平台开启」执行体（issue #64 T02）。
import { platformApi, serializeProxyEnabled } from "../../services/api";

/** 「为全部平台开启」执行体（issue #64 T02）。
 * 逐平台写 extra.proxy_enabled=true（保留其余键），返回成功/失败数；写完广播平台列表刷新
 * （usePlatformsState 监听 aidog-platforms-changed）。单平台失败不中断其余。
 * shortcut: 逐平台「读 extra → 改写 → 整串写回」与并发编辑同一平台存在覆盖竞态（用户在
 * 平台编辑页保存会丢本函数之外的键）；彻底解决需后端批量命令（单事务 + 原子 read-modify-write），
 * 后端就绪前靠「写完广播刷新」缓解前端各处缓存的旧值。 */
export async function enableProxyForAllPlatforms(): Promise<{ ok: number; failed: number }> {
  const list = (await platformApi.list()) || [];
  let ok = 0;
  let failed = 0;
  for (const p of list) {
    try {
      await platformApi.update({ id: p.id, extra: serializeProxyEnabled(p.extra ?? "", true) });
      ok++;
    } catch {
      failed++;
    }
  }
  window.dispatchEvent(new Event("aidog-platforms-changed"));
  return { ok, failed };
}
