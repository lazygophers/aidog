// groupsChanged — 平台成员变更通知单一入口（issue #58 票 09，取代手写「三连」）。
// 旧三连：handleGroupsChanged() + groupsReloadRef.current?.() + dispatchEvent("aidog-groups-changed")。
// 新链路：写路径只调本入口 → 广播一次事件 → 分组页监听端（Groups.tsx）按 rowsChanged
// 决定是否整页重载行，并统一触发 onGroupsChanged（groupDetails 重建）。

export interface MembershipChangeOpts {
  /** false = 平台行不变（拖入分组特例）：不触发分组页整页行重载，仅 groupDetails + 广播。缺省 true。 */
  rowsChanged?: boolean;
}

/** 写路径（save / delete / purge / 批量创建 / 拖入）成员变更后调用一次即可。 */
export function notifyPlatformMembershipChanged(opts?: MembershipChangeOpts): void {
  window.dispatchEvent(new CustomEvent("aidog-groups-changed", {
    detail: { rowsChanged: opts?.rowsChanged ?? true },
  }));
}
