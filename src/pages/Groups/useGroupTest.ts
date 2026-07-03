import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { modelTestApi, type GroupPlatformDetail } from "../../services/api";
import { BATCH_TEST_CONCURRENCY, type GroupTestRow } from "../../domains/groups";

export interface GroupTestState {
  groupId: number;
  groupName: string;
  rows: GroupTestRow[];
  running: boolean;
}

/**
 * 分组一键测试：有界并发跑 model_test，结果面板逐行实时刷新。
 * 不复用 usePlatformCards 的 testingId/testResults —— 那是单卡态，组级面板独立维护行集合。
 */
export function useGroupTest() {
  const { t } = useTranslation();
  const [groupTest, setGroupTest] = useState<GroupTestState | null>(null);

  const handleTestGroup = useCallback(async (group: { id: number; name: string }, gps: GroupPlatformDetail[]) => {
    // 只测启用平台，跳过 disabled / auto_disabled（不出现在 rows、不发测试请求）
    const enabledGps = gps.filter(gp => gp.platform.status === "enabled");
    if (enabledGps.length === 0) return;
    const rows: GroupTestRow[] = enabledGps.map(gp => ({
      platformId: gp.platform.id, name: gp.platform.name, status: "pending",
    }));
    setGroupTest({ groupId: group.id, groupName: group.name, rows, running: true });
    // groupId guard：面板被中途关闭（groupTest=null）或被新一轮测试取代时，在途写回 no-op，不复活面板。
    const patchRow = (idx: number, patch: Partial<GroupTestRow>) =>
      setGroupTest(prev =>
        prev && prev.groupId === group.id
          ? { ...prev, rows: prev.rows.map((r, i) => i === idx ? { ...r, ...patch } : r) }
          : prev);
    const testOne = async (idx: number) => {
      const gp = enabledGps[idx];
      patchRow(idx, { status: "testing" });
      const defaultModel = gp.platform.models.default || gp.platform.available_models[0] || "";
      const start = Date.now();
      let success = false;
      try {
        const r = await modelTestApi.test({ platform_id: gp.platform.id, model: defaultModel });
        const durationMs = Date.now() - start;
        success = r.success;
        patchRow(idx, r.success
          ? { status: "ok", durationMs }
          : { status: "fail", durationMs, error: r.error || t("platform.testFail", "测试失败") });
      } catch (err: any) {
        patchRow(idx, { status: "fail", durationMs: Date.now() - start, error: err?.message || t("platform.testFail", "测试失败") });
      }
      // 携带 success：监听方（usePlatformCards/Platforms）据此写 testResults → 单卡 health 走 manual 分支即时变绿/红；
      // 同时驱动「最近测试」徽章刷新（不在 Groups 维护 lastTestMap）
      window.dispatchEvent(new CustomEvent("aidog-platform-test-completed", { detail: { platformId: gp.platform.id, success } }));
    };
    // 有界并发：共享游标 next，启 N 个 worker，各自循环领取下一个 idx 直到耗尽。
    let next = 0;
    const worker = async () => {
      while (next < enabledGps.length) {
        const idx = next++;
        await testOne(idx);
      }
    };
    const pool = Array.from(
      { length: Math.min(BATCH_TEST_CONCURRENCY, enabledGps.length) },
      () => worker(),
    );
    await Promise.all(pool);
    setGroupTest(prev => prev && prev.groupId === group.id ? { ...prev, running: false } : prev);
  }, [t]);

  return { groupTest, setGroupTest, handleTestGroup };
}
