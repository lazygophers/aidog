import { useEffect, useLayoutEffect, useRef } from "react";

/**
 * 列表 ⇄ 详情整页切换时保住列表滚动位：滚动容器是 App 的 <main>，列表卸载后其 scrollTop 会被夹到 0。
 * 列表态持续记录 scrollTop；进详情归零；回列表恢复。
 */
export function useDetailScroll(inDetail: boolean) {
  const saved = useRef(0);
  useEffect(() => {
    if (inDetail) return;
    const main = document.querySelector("main");
    if (!main) return;
    const onScroll = () => { saved.current = main.scrollTop; };
    main.addEventListener("scroll", onScroll, { passive: true });
    return () => main.removeEventListener("scroll", onScroll);
  }, [inDetail]);
  useLayoutEffect(() => {
    const main = document.querySelector("main");
    if (main) main.scrollTop = inDetail ? 0 : saved.current;
  }, [inDetail]);
}
