import { isPlainObject } from "./editors/ImportDiff";

/**
 * 按选中的 dot-path 集合，把 source 中对应值深合并进 config 的克隆。
 * 未选中的子键保留 config 原值；path 在 source 中不存在（"removed" 类差异）→ 从结果里删掉该 key。
 * 自 Settings.tsx applyImport 内联逻辑外迁（纯函数，便于单测，无组件/state 依赖）。
 */
export function applySelectedPaths(
  config: Record<string, any>,
  source: Record<string, any>,
  selectedPaths: Set<string>,
): Record<string, any> {
  const next: Record<string, any> = JSON.parse(JSON.stringify(config));
  for (const path of selectedPaths) {
    const segs = path.split(".");
    // Resolve incoming value by walking source along the path.
    let incoming: any = source;
    let found = true;
    for (const s of segs) {
      if (incoming != null && typeof incoming === "object" && s in incoming) {
        incoming = incoming[s];
      } else {
        incoming = undefined;
        found = false;
        break;
      }
    }
    // Write into next at the path, creating intermediate objects as needed.
    let cursor = next;
    for (let i = 0; i < segs.length - 1; i++) {
      const s = segs[i];
      if (!isPlainObject(cursor[s])) cursor[s] = {};
      cursor = cursor[s];
    }
    const leaf = segs[segs.length - 1];
    if (found) {
      cursor[leaf] = incoming;
    } else {
      delete cursor[leaf];
    }
  }
  return next;
}
