// ponytail: 自 StatusLineSection.tsx L290-790 外迁，零逻辑变更。
// 仅消费 useStatusLinePanel 的 state + actions，渲染 JSX。

import { useTranslation } from "react-i18next";
import { IconClose, IconMenu, IconEdit } from "../../../icons";
import { SortableList } from "../../../SortableList";
import {
  SEGMENT_DEF_MAP,
  SEGMENT_CATEGORIES,
  isRowLeaderSeg,
} from "../../statusline-gen";
import { F, S } from "../tokens";
import { Toggle, Hint } from "../_shared";
import { previewColor, StatusLinePreview } from "./preview";
import { SegmentEditModal } from "./SegmentEditModal";
import { useStatusLinePanel, type ScriptType } from "./useStatusLinePanel";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export function StatusLinePanel({
  config,
  updateField,
  scriptType,
  t,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
  scriptType: ScriptType;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const {
    isMain, aidogKey, stored, enabled, mode, customCommand, segments, scriptPreview,
    showScript, setShowScript, saving, editSeg, setEditSeg, showAddMenu, setShowAddMenu,
    handleToggle, updateSegments, deleteRow, handleSave, handleApplyCustom,
    switchMode, addSegment, addRow, resetToDefaultLayout, cycleRowAlign,
  } = useStatusLinePanel({ config, updateField, scriptType });

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {/* Enable toggle */}
      <div style={{
        display: "flex", alignItems: "center", gap: 12,
        padding: "12px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
      }}>
        <Toggle active={enabled} onChange={handleToggle} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-primary)" }}>
            {isMain ? t("statusline.useBuiltin", "使用内置状态栏") : t("statusline.useBuiltinSubagent", "使用内置子代理状态栏")}
          </div>
          <Hint>{isMain
            ? t("statusline.builtinDesc", "开启后 aidog 生成脚本到 ~/.aidog/aidog-statusline.sh")
            : t("statusline.builtinSubagentDesc", "开启后 aidog 生成脚本到 ~/.aidog/aidog-subagent-statusline.sh")}</Hint>
        </div>
        {enabled && (
          <span style={{
            fontSize: F.small, fontWeight: 600, color: "var(--color-success)",
            padding: "2px 8px", background: "color-mix(in srgb, var(--color-success) 12%, transparent)", borderRadius: "var(--radius-sm)",
          }}>● {t("statusline.enabled", "已启用")}</span>
        )}
      </div>

      {enabled && (
        <>
          {/* Mode selector: builtin structured segments vs custom native command */}
          <div style={{ display: "flex", gap: 6 }}>
            {(["builtin", "custom"] as const).map(m => {
              const active = mode === m;
              return (
                <Button variant="outline" key={m} type="button"
                  style={{
                    flex: 1, padding: "8px 12px", fontSize: F.body, fontWeight: active ? 600 : 400,
                    color: active ? "var(--accent)" : "var(--text-secondary)",
                    background: active ? "var(--accent-subtle, rgba(0,122,255,0.1))" : "transparent",
                    border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
                    borderRadius: "var(--radius-sm)", cursor: "pointer",
                  }}
                  onClick={() => switchMode(m)}>
                  {m === "builtin"
                    ? t("statusline.modeBuiltin", "内置结构化")
                    : t("statusline.modeCustom", "自定义脚本")}
                </Button>
              );
            })}
          </div>
        </>
      )}

      {enabled && mode === "custom" && (
        <div style={{
          padding: "12px 16px", background: "var(--bg-surface)", borderRadius: "var(--radius-md)",
          border: "1px solid var(--border)", display: "flex", flexDirection: "column", gap: 12,
        }}>
          <Hint>{t("statusline.customDesc", "按原生 statusLine 格式分字段填写，写入 settings 的 command 字段，不生成 aidog 脚本")}</Hint>
          {/* type — 固定 command，只读展示 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <label style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("statusline.customType", "类型")}</label>
            <Input  readOnly value="command"
              style={{ fontSize: F.body, padding: S.inputPad, width: 140, opacity: 0.7, fontFamily: '"SF Mono", "Fira Code", monospace' }} />
          </div>
          {/* command — 脚本路径 / 命令 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <label style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("statusline.customCommand", "命令 / 脚本路径")}</label>
            <Input  style={{ fontSize: F.body, padding: S.inputPad }}
              value={customCommand}
              placeholder={t("statusline.customPlaceholder", "~/.claude/my-statusline.sh 或 inline 命令")}
              onChange={(e) => updateField(aidogKey, { ...stored, customCommand: e.target.value })} />
            <Hint>{t("statusline.customCommandDesc", "支持绝对路径、~ 路径或内联命令")}</Hint>
          </div>
          <div style={{ display: "flex", justifyContent: "flex-end" }}>
            <Button variant="default"  style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={handleApplyCustom}>
              {t("statusline.applyCustom", "应用自定义脚本")}
            </Button>
          </div>
        </div>
      )}

      {enabled && mode === "builtin" && (
        <>
          {/* Preview */}
          <div style={{
            padding: "12px 16px", background: "var(--bg-surface)", borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
          }}>
            <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 6 }}>{t("statusline.preview")}</div>
            <div style={{
              fontFamily: '"SF Mono", "Fira Code", monospace', fontSize: F.body,
              color: "var(--text-primary)", lineHeight: 1.6,
            }}>
              <StatusLinePreview segments={segments} empty={t("statusline.previewEmpty")} />
            </div>
          </div>

          {/* ── Drag-sortable segment list (shared by main & subagent) ── */}
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <SortableList
                items={segments}
                onReorder={updateSegments}
                renderItem={(seg, handle) => {
                  const def = SEGMENT_DEF_MAP.get(seg.type);
                  if (!def) return null;
                  const leader = isRowLeaderSeg(segments, seg.id);
                  const segColor = previewColor(seg);
                  return (
                    <div style={{ marginBottom: 6 }}>
                    {/* Row-leader bar: new-line marker + alignment */}
                    {leader && (
                      <div style={{
                        display: "flex", alignItems: "center", gap: 8,
                        padding: "2px 4px 4px", fontSize: F.hint, color: "var(--text-tertiary)",
                      }}>
                        <span style={{ fontWeight: 600 }}>{t("statusline.rowLabel")}</span>
                        <Button variant="ghost" type="button" 
                          style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--accent)" }}
                          onClick={() => cycleRowAlign(seg.id)}
                          title={t("statusline.rowAlign")}>
                          {t(`statusline.align.${seg.align ?? "left"}`)}
                        </Button>
                        <Button variant="ghost" type="button" 
                          style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--text-tertiary)" }}
                          onClick={() => deleteRow(seg.id)}
                          title={t("statusline.deleteRow", "删除整行")}>
                          {t("statusline.deleteRow", "删除整行")}
                        </Button>
                      </div>
                    )}
                    <div className="glass-surface" style={{
                      display: "flex", alignItems: "center", gap: 10,
                      padding: "10px 12px",
                      borderRadius: "var(--radius-md)",
                      opacity: seg.enabled ? 1 : 0.45,
                      border: handle.isDragging ? "1px solid var(--accent)" : "1px solid var(--border)",
                      boxShadow: handle.isDragging ? "0 6px 20px rgba(0,0,0,0.18)" : "none",
                      transition: "opacity 150ms, border-color 150ms",
                    }}>
                      {/* Drag handle (only this element starts the drag) */}
                      <span
                        ref={handle.ref}
                        {...handle.attributes}
                        {...handle.listeners}
                        style={{
                          color: "var(--text-tertiary)", fontSize: F.body,
                          cursor: handle.isDragging ? "grabbing" : "grab",
                          userSelect: "none", touchAction: "none",
                          padding: "0 2px", lineHeight: 1,
                        }}
                        title={t("statusline.dragSort", "拖动排序")}
                      ><IconMenu size={15} /></span>
                      {/* Toggle */}
                      <Toggle active={seg.enabled} onChange={(v) => {
                        const next = segments.map(s => s.id === seg.id ? { ...s, enabled: v } : s);
                        updateSegments(next);
                      }} />
                      {/* Name */}
                      <span style={{ fontSize: F.body, fontWeight: 600, color: "var(--text-primary)", flexShrink: 0 }}>
                        {t(`statusline.seg.${def.type}.name`, def.name)}
                      </span>
                      {/* Inline preview (colored) */}
                      <span style={{
                        flex: 1, fontSize: F.hint,
                        color: segColor ?? "var(--text-tertiary)",
                        fontFamily: '"SF Mono", "Fira Code", monospace',
                        overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                      }}>
                        {def.toPreview({ ...def.defaultOptions, ...seg.options })}
                      </span>
                      {/* Break-to-new-line toggle (moves segment between rows) */}
                      <Button variant="ghost" type="button" 
                        style={{
                          width: 24, height: 24, minWidth: 24, fontSize: F.hint,
                          color: seg.newline ? "var(--accent)" : "var(--text-tertiary)",
                        }}
                        title={t("statusline.toggleNewline")}
                        onClick={() => updateSegments(segments.map(s => s.id === seg.id ? { ...s, newline: !s.newline } : s))}>↵</Button>
                      {/* Edit button */}
                      <Button variant="ghost" type="button" 
                        style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--accent)" }}
                        onClick={() => setEditSeg({ ...seg })}>
                        <IconEdit size={13} />
                      </Button>
                      {/* Delete */}
                      <Button variant="ghost" type="button" 
                        style={{ width: 24, height: 24, minWidth: 24, fontSize: F.hint, color: "var(--text-tertiary)" }}
                        onClick={() => updateSegments(segments.filter((s) => s.id !== seg.id))}>
                        <IconClose size={13} />
                      </Button>
                    </div>
                    </div>
                  );
                }}
              />

              {/* Add segment / row */}
              <div style={{ position: "relative", display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
                <Button variant="ghost"  style={{ fontSize: F.body, padding: "6px 14px", marginRight: "auto", color: "var(--text-tertiary)" }}
                  onClick={resetToDefaultLayout}
                  title={t("statusline.resetLayoutHint", "恢复内置默认 3 行布局")}>
                  {t("statusline.resetLayout", "恢复默认布局")}
                </Button>
                <Button variant="ghost"  style={{ fontSize: F.body, padding: "6px 14px" }}
                  onClick={addRow}>
                  {t("statusline.addRow")}
                </Button>
                <Button variant="ghost"  style={{ fontSize: F.body, padding: "6px 14px" }}
                  onClick={() => setShowAddMenu(!showAddMenu)}>
                  {t("statusline.addSegment")}
                </Button>
                {showAddMenu && (
                  <div style={{
                    position: "absolute", bottom: "100%", right: 0, zIndex: 100,
                    background: "var(--bg-surface)", border: "1px solid var(--border)",
                    borderRadius: "var(--radius-md)", padding: 4,
                    maxHeight: 360, overflow: "auto", minWidth: 280,
                    boxShadow: "0 8px 32px rgba(0,0,0,0.2)",
                  }}>
                    {SEGMENT_CATEGORIES.map(cat => (
                      <div key={cat.id}>
                        <div style={{
                          padding: "6px 12px 2px", fontSize: F.small, fontWeight: 600,
                          color: "var(--text-tertiary)", textTransform: "uppercase", letterSpacing: 0.4,
                        }}>{t(`statusline.segCat.${cat.id}`, cat.label)}</div>
                        {cat.types.map(type => {
                          const def = SEGMENT_DEF_MAP.get(type);
                          if (!def) return null;
                          return (
                            <Button variant="outline" key={def.type} type="button" style={{
                              display: "block", width: "100%", textAlign: "left",
                              padding: "6px 12px", fontSize: F.body,
                              background: "transparent", border: "none", borderRadius: "var(--radius-sm)",
                              cursor: "pointer", color: "var(--text-primary)",
                            }}
                              onMouseEnter={(e) => { e.currentTarget.style.background = "var(--bg-glass)"; }}
                              onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
                              onClick={() => addSegment(def.type)}>
                              <span style={{ fontWeight: 500 }}>{t(`statusline.seg.${def.type}.name`, def.name)}</span>
                              <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginLeft: 8 }}>{t(`statusline.seg.${def.type}.desc`, def.desc)}</span>
                            </Button>
                          );
                        })}
                      </div>
                    ))}
                  </div>
                )}
              </div>
          </div>


          {/* Script preview (collapsible) */}
          <div style={{
            padding: "10px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
          }}>
            <Button variant="ghost" type="button" 
              style={{ fontSize: F.body, padding: "4px 8px", display: "flex", alignItems: "center", gap: 4, width: "100%", justifyContent: "space-between" }}
              onClick={() => setShowScript(!showScript)}>
              <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                <span style={{ transform: showScript ? "rotate(90deg)" : "rotate(0deg)", transition: "transform 150ms", display: "inline-block" }}>▶</span>
                {t("statusline.scriptPreview", "脚本预览")}
              </span>
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)", fontFamily: '"SF Mono", "Fira Code", monospace' }}>
                ~/.aidog/aidog-{scriptType === "subagent" ? "subagent-" : ""}statusline.sh
              </span>
            </Button>
            {showScript && (
              <pre style={{
                fontFamily: '"SF Mono", "Fira Code", monospace',
                fontSize: F.hint, lineHeight: 1.6,
                background: "var(--bg-surface)", borderRadius: "var(--radius-sm)",
                padding: 12, overflow: "auto", whiteSpace: "pre",
                color: "var(--text-primary)", margin: 0, marginTop: 8,
              }}>
                {scriptPreview}
              </pre>
            )}
          </div>

          {/* Apply button */}
          <div style={{ display: "flex", justifyContent: "flex-end" }}>
            <Button variant="default"  style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={handleSave} disabled={saving}>
              {saving ? t("statusline.generating", "生成中…") : t("statusline.applyGenerate", "应用并生成脚本")}
            </Button>
          </div>
        </>
      )}

      {/* Edit modal */}
      {editSeg && (
        <SegmentEditModal
          segment={editSeg}
          isRowLeader={isRowLeaderSeg(segments, editSeg.id)}
          t={t}
          onClose={() => setEditSeg(null)}
          onSave={(patch) => {
            const idx = segments.findIndex(s => s.id === editSeg.id);
            if (idx >= 0) {
              const next = [...segments];
              next[idx] = { ...next[idx], ...patch };
              updateSegments(next);
            }
            setEditSeg(null);
          }}
        />
      )}
    </div>
  );
}
