// useBatchAction — Groups 四套批量操作（删除 / 覆盖模型 / 改状态 / 移组）共用的执行样板
// （issue #58 票 08）：target + busy + 成功/失败 toast + silentReload 刷新 + 关 modal。
// 行为与抽前逐行同形：成功 → 清 target → 刷新 → onSuccess → toast(3s 清)；
// 失败 → toast(3s 清) → 清 target；busy 在 finally 复位。API 前置分支（移组独占校验）留调用方。
import { useCallback, useState } from "react";

export interface BatchActionDeps {
  silentReload: () => void;
  onGroupsChanged?: () => void;
  onToast?: (toast: { text: string; ok: boolean } | null) => void;
}

export interface BatchRunOpts<R extends { applied: number }> {
  api: () => Promise<R>;
  /** 成功文案（含 applied 计数插值）。 */
  doneText: (applied: number) => string;
  /** 失败 toast 前缀（拼 `: ${e}`）。 */
  failPrefix: string;
  /** 成功后额外副作用（删除 → onPlatformDeleted(ids)；其余 → batchDoneSignal +1）。 */
  onSuccess?: () => void;
}

export function useBatchAction<T>(deps: BatchActionDeps) {
  const [target, setTarget] = useState<T | null>(null);
  const [busy, setBusy] = useState(false);

  const runBatch = useCallback(async (opts: BatchRunOpts<{ applied: number }>): Promise<void> => {
    setBusy(true);
    try {
      const report = await opts.api();
      setTarget(null);
      deps.silentReload();
      deps.onGroupsChanged?.();
      opts.onSuccess?.();
      deps.onToast?.({ text: opts.doneText(report.applied), ok: true });
      setTimeout(() => deps.onToast?.(null), 3000);
    } catch (e) {
      console.error(e);
      deps.onToast?.({ text: `${opts.failPrefix}: ${e}`, ok: false });
      setTimeout(() => deps.onToast?.(null), 3000);
      setTarget(null);
    } finally {
      setBusy(false);
    }
  }, [deps.silentReload, deps.onGroupsChanged, deps.onToast]);

  return { target, setTarget, busy, runBatch };
}
