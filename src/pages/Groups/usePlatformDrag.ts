import { useCallback, useRef, useState } from "react";
import type { Platform, GroupDetail, GroupPlatformDetail } from "../../services/api";
import { groupDetailApi } from "../../services/api";

type DndPayload = { pid: number; fromGid: number };

export interface PlatformDragApi {
  details: GroupDetail[];
  platforms: Platform[];
  setDetails: React.Dispatch<React.SetStateAction<GroupDetail[]>>;
  load: () => Promise<void> | void;
  onToast?: (toast: { text: string; ok: boolean } | null) => void;
  onGroupsChanged?: () => void;
}

export interface PlatformDragResult {
  dropIndicator: { gid: number; idx: number } | null;
  dragOverGroup: number | null;
  onPlatPointerDown: (ev: React.PointerEvent, pid: number, gid: number) => void;
}

/**
 * 分组展开区平台拖拽（pointer 事件驱动，不依赖 HTML5 drop —— WKWebView 下 drop 不可靠）。
 * 不与 dnd-kit 分组排序冲突：平台拖拽把手与分组排序把手是不同 DOM 节点。
 *
 * 用 ref 镜像 details/platforms/load/onToast/onGroupsChanged，避免 useCallback([]) 闭包陈旧；
 * 父组件每次 render 同步刷新 ref（无 re-subscribe 开销），pointer 监听读到最新值。
 */
export function usePlatformDrag(api: PlatformDragApi): PlatformDragResult {
  const { setDetails } = api;
  const [dropIndicator, setDropIndicator] = useState<{ gid: number; idx: number } | null>(null);
  const [dragOverGroup, setDragOverGroup] = useState<number | null>(null);

  // ref 镜像：稳定的 pointer handler 闭包读取最新 props（避免每次 deps 变化重新挂监听）
  const apiRef = useRef(api);
  apiRef.current = api;

  // platDragRef：可变拖拽态（不用 state，避免 pointermove 频繁 rerender）
  const platDragRef = useRef<{ payload: DndPayload | null; active: boolean; startX: number; startY: number }>({
    payload: null, active: false, startX: 0, startY: 0,
  });

  const computeDropIdx = (zoneEl: HTMLElement, clientY: number): number => {
    const cards = zoneEl.querySelectorAll<HTMLElement>("[data-gp-id]");
    for (let i = 0; i < cards.length; i++) {
      const r = cards[i].getBoundingClientRect();
      if (clientY < r.top + r.height / 2) return i;
    }
    return cards.length;
  };

  // 从 elementFromPoint 反查目标分组 + 插入位（命中分组 wrapper 的 data-group-id）。
  const hitTestZone = (clientX: number, clientY: number): { gid: number; idx: number; zoneEl: HTMLElement } | null => {
    const el = document.elementFromPoint(clientX, clientY) as HTMLElement | null;
    if (!el) return null;
    const zoneEl = el.closest<HTMLElement>("[data-group-id]");
    if (!zoneEl) return null;
    const gid = Number(zoneEl.dataset.groupId);
    if (!Number.isFinite(gid)) return null;
    return { gid, idx: computeDropIdx(zoneEl, clientY), zoneEl };
  };

  const commitPlatDrop = (gid: number, idx: number, payload: DndPayload) => {
    const { details, platforms, load, onToast, onGroupsChanged } = apiRef.current;
    // 从 details 推导目标分组当前平台顺序
    const fullPlats = (details.find(d => d.group.id === gid)?.platforms ?? [])
      .map(gp => platforms.find(pp => pp.id === gp.platform.id))
      .filter((pp): pp is Platform => !!pp);

    if (payload.fromGid === gid) {
      // 组内重排
      const ids = fullPlats.map(p => p.id);
      const fromIdx = ids.indexOf(payload.pid);
      if (fromIdx < 0) return;
      let target = idx;
      if (fromIdx < idx) target = idx - 1; // 移除拖动项后位置左移
      if (target === fromIdx) return;
      const reordered = ids.filter(id => id !== payload.pid);
      reordered.splice(target, 0, payload.pid);
      setDetails(prev => prev.map(d => d.group.id !== gid ? d : {
        ...d,
        platforms: reordered.map((id, i) => {
          const gp = d.platforms.find(g => g.platform.id === id)!;
          return { ...gp, priority: i + 1 };
        }),
      }));
      groupDetailApi.reorderPlatforms(gid, reordered).catch(console.error);
    } else {
      if (payload.fromGid === 0) {
        // 从未分组列表拖入（fromGid=0，无源组）: 构造新明细乐观插入目标组
        const plat = platforms.find(pp => pp.id === payload.pid);
        if (plat) {
          setDetails(prev => prev.map(d => {
            if (d.group.id !== gid) return d;
            const newGp: GroupPlatformDetail = { platform: plat, priority: d.platforms.length + 1, weight: 1 };
            const gps = [...d.platforms];
            gps.splice(Math.min(idx, gps.length), 0, newGp);
            return { ...d, platforms: gps };
          }));
        }
        const gname = details.find(d => d.group.id === gid)?.group.name ?? `#${gid}`;
        groupDetailApi.movePlatform(payload.pid, 0, gid)
          .then(() => {
            onToast?.({ text: `已加入分组 ${gname}`, ok: true });
            load(); onGroupsChanged?.();
          })
          .catch((err) => {
            console.error("[aidog-dnd] movePlatform failed", err);
            onToast?.({ text: `加入分组失败: ${err}`, ok: false });
            load(); // 回滚乐观插入
          });
      } else {
        // 跨组移动
        let movedGp: GroupPlatformDetail | undefined;
        setDetails(prev => {
          const next = prev.map(d => {
            if (d.group.id === payload.fromGid) {
              const gps = d.platforms.filter(g => {
                if (g.platform.id === payload.pid) { movedGp = g; return false; }
                return true;
              });
              return { ...d, platforms: gps };
            }
            return d;
          });
          if (!movedGp) return next;
          return next.map(d => {
            if (d.group.id !== gid) return d;
            const newGp = { ...movedGp!, priority: d.platforms.length + 1 };
            const gps = [...d.platforms];
            const insertAt = Math.min(idx, gps.length);
            gps.splice(insertAt, 0, newGp);
            return { ...d, platforms: gps };
          });
        });
        groupDetailApi.movePlatform(payload.pid, payload.fromGid, gid)
          .then(() => load()).catch(console.error);
      }
    }
  };

  // 拖拽阈值（px）：pointermove 累计位移超过才视为拖拽，避免误触把手当点击。
  const PLAT_DRAG_THRESHOLD = 4;

  // pointermove：超阈后置 active，每帧 hit-test 更新 dropIndicator/dragOverGroup。
  const onPlatPointerMove = (ev: PointerEvent) => {
    const st = platDragRef.current;
    if (!st.payload) return;
    if (!st.active) {
      if (Math.abs(ev.clientX - st.startX) + Math.abs(ev.clientY - st.startY) < PLAT_DRAG_THRESHOLD) return;
      st.active = true;
    }
    ev.preventDefault();
    const hit = hitTestZone(ev.clientX, ev.clientY);
    if (!hit) {
      setDragOverGroup(prev => (prev === null ? prev : null));
      setDropIndicator(prev => (prev === null ? prev : null));
      return;
    }
    setDragOverGroup(prev => (prev === hit.gid ? prev : hit.gid));
    setDropIndicator(prev => (prev?.gid === hit.gid && prev?.idx === hit.idx) ? prev : { gid: hit.gid, idx: hit.idx });
  };

  // pointerup：落定（仅当超阈成拖拽且命中目标组）后清理监听与状态。
  const onPlatPointerUp = (ev: PointerEvent) => {
    const st = platDragRef.current;
    const payload = st.payload;
    document.removeEventListener("pointermove", onPlatPointerMove);
    document.removeEventListener("pointerup", onPlatPointerUp);
    document.removeEventListener("pointercancel", onPlatPointerUp);
    st.payload = null;
    const wasActive = st.active;
    st.active = false;
    setDropIndicator(null);
    setDragOverGroup(null);
    if (!payload || !wasActive) return;
    const hit = hitTestZone(ev.clientX, ev.clientY);
    if (!hit) return;
    commitPlatDrop(hit.gid, hit.idx, payload);
  };

  // pointerdown 起拖：记录 payload + 起点，挂 document 级 move/up 监听（elementFromPoint 跨组生效）。
  // useCallback([]) — 仅引用 ref（platDragRef）和不变函数（onPlatPointerMove/Up），无 state 依赖。
  const onPlatPointerDown = useCallback((ev: React.PointerEvent, pid: number, gid: number) => {
    ev.preventDefault();
    ev.stopPropagation(); // 不冒泡到 dnd-kit 分组排序把手
    const st = platDragRef.current;
    st.payload = { pid, fromGid: gid };
    st.active = false;
    st.startX = ev.clientX;
    st.startY = ev.clientY;
    document.addEventListener("pointermove", onPlatPointerMove);
    document.addEventListener("pointerup", onPlatPointerUp);
    document.addEventListener("pointercancel", onPlatPointerUp);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { dropIndicator, dragOverGroup, onPlatPointerDown };
}

