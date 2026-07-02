import type { TFunction } from "i18next";
import type { Platform } from "../../services/api";
import { SortableList } from "../../components/SortableList";
import { IconClose } from "../../components/icons";

/** Row model for the sortable selected-platforms list (stable string id for @dnd-kit). */
export interface SortablePlatform {
  id: string;
  platformId: number;
}

// ── Design tokens (shared by edit/create views; mirror of F/S) ──
export const PICKER_F = { label: 15, body: 15, hint: 13, small: 12 } as const;

/**
 * 关联平台选择器：已选平台拖拽重排（顺序=优先级）+ 上下移 + 移除 + 下拉添加。
 * 编辑视图与创建视图共用，确保两处交互/组件一致（创建时分组尚无 id，故纯受控 platformIds）。
 */
export function PlatformPicker({ platformIds, options, onChange, t }: {
  platformIds: number[];
  options: Platform[];
  onChange: (ids: number[]) => void;
  t: TFunction;
}) {
  return (
    <>
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <SortableList<SortablePlatform>
          items={platformIds.map(pid => ({ id: String(pid), platformId: pid }))}
          onReorder={next => onChange(next.map(row => row.platformId))}
          renderItem={(row, handle) => {
            const pid = row.platformId;
            const i = platformIds.indexOf(pid);
            const p = options.find(pp => pp.id === pid);
            if (!p) return null;
            return (
              <div style={{
                display: "flex", alignItems: "center", gap: 10,
                padding: "8px 12px", borderRadius: "var(--radius-sm)",
                background: "var(--bg-glass)",
                border: "1px solid var(--border)",
                marginBottom: 4,
                transition: "opacity 0.15s, border-color 0.15s",
              }}>
                <span
                  ref={handle.ref}
                  {...handle.attributes}
                  {...handle.listeners}
                  title={t("group.dragToReorder", "拖动排序")}
                  style={{
                    cursor: "grab", color: "var(--text-tertiary)", fontSize: 14,
                    lineHeight: 1, userSelect: "none", flexShrink: 0, touchAction: "none",
                  }}
                >⠿</span>
                <span style={{ fontSize: PICKER_F.hint, color: "var(--text-tertiary)", width: 20, textAlign: "center" }}>
                  {i + 1}
                </span>
                <span style={{
                  width: 28, height: 28, borderRadius: "var(--radius-sm)",
                  display: "flex", alignItems: "center", justifyContent: "center",
                  background: "var(--accent-subtle)", color: "var(--accent)",
                  fontSize: 11, fontWeight: 700, flexShrink: 0,
                }}>
                  {p.platform_type.slice(0, 2).toUpperCase()}
                </span>
                <span style={{ flex: 1, fontSize: PICKER_F.body, fontWeight: 500 }}>{p.name}</span>
                <button type="button" className="btn btn-ghost btn-icon" style={{ width: 24, height: 24, minWidth: 24, padding: 0 }}
                  disabled={i === 0}
                  onClick={() => {
                    const ids = [...platformIds];
                    [ids[i - 1], ids[i]] = [ids[i], ids[i - 1]];
                    onChange(ids);
                  }}>
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                    <path d="M5 2v6M2 5l3-3 3 3" />
                  </svg>
                </button>
                <button type="button" className="btn btn-ghost btn-icon" style={{ width: 24, height: 24, minWidth: 24, padding: 0 }}
                  disabled={i === platformIds.length - 1}
                  onClick={() => {
                    const ids = [...platformIds];
                    [ids[i], ids[i + 1]] = [ids[i + 1], ids[i]];
                    onChange(ids);
                  }}>
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                    <path d="M5 8V2M2 5l3 3 3-3" />
                  </svg>
                </button>
                <button type="button" onClick={() => onChange(platformIds.filter(id => id !== pid))} style={{
                  background: "none", border: "none", cursor: "pointer",
                  color: "var(--text-tertiary)", fontSize: PICKER_F.small, padding: 4, lineHeight: 1,
                }}><IconClose size={12} /></button>
              </div>
            );
          }}
        />
      </div>
      {platformIds.length < options.length && (
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <select className="input" style={{ fontSize: PICKER_F.hint, padding: "6px 10px", flex: 1 }}
            onChange={e => {
              const pid = Number(e.target.value);
              if (e.target.value && !platformIds.includes(pid)) {
                onChange([...platformIds, pid]);
              }
              e.target.value = "";
            }}>
            <option value="">{t("group.addPlatform", "+ 添加平台")}</option>
            {options
              .filter(p => !platformIds.includes(p.id))
              .map(p => <option key={p.id} value={p.id}>{p.name} ({p.platform_type})</option>)}
          </select>
        </div>
      )}
    </>
  );
}
