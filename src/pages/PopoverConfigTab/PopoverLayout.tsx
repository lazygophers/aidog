// ─── 浮窗配置页 JSX 区块（说明 / 编辑器 / 预览）──
// 自 PopoverConfigTab 主组件 JSX 外迁（arch 阶段6 S5），state/actions 在 usePopoverConfig。
import {
  DndContext,
  pointerWithin,
  DragOverlay,
} from "@dnd-kit/core";
import {
  renderGrid,
  type PopoverData,
} from "../../components/PopoverCards";
import type { PopoverConfigData } from "./usePopoverConfig";
import { TYPE_LABELS, GROUP_TYPES } from "./constants";
import { RowContainer } from "./RowContainer";
import { SortableCard } from "./SortableCard";
import { CardEditor } from "./CardEditor";
import { Button } from "@/components/ui/button";

export function PopoverLayout(d: PopoverConfigData) {
  const {
    t, loading, message, config, groups, groupDetails, platforms,
    todayStats, platformToday, statsCtx, activeItem,
    showAddMenu, setShowAddMenu, rowGroups, colsForRow, availableTypes,
    sensors, toggleVisible, removeItem, updateItem, addItem, setRowCols,
    showLayoutHint, handleDragStart, handleDragOver, handleDragEnd,
    previewValue, trendSummary,
  } = d;

  if (loading) {
    return <div className="text-secondary" style={{ fontSize: 13, padding: 20 }}>{t("common.loading", "加载中...")}</div>;
  }

  // 实时预览数据：用 draft config + 已轮询 stats 合成 PopoverData（与真实浮窗共用 renderGrid）。
  const previewData: PopoverData = {
    config,
    entries: [], // platform_balance 余额行来自托盘配置，预览不可得，此处留空（卡片自隐）。
    today_stats: todayStats ?? { tokens: 0, input_tokens: 0, output_tokens: 0, cache_tokens: 0, cache_rate: 0, cost: 0, total_requests: 0 },
    platform_today: platformToday,
    proxy_running: true,
    proxy_port: 0,
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* 说明 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("popover.title", "浮窗展示")}</div>
        <div className="text-secondary" style={{ fontSize: 12 }}>
          {t("popover.descGrid", "托盘浮窗内容，可显隐、二维拖拽布局、设每行列数、每卡尺寸与颜色。")}
        </div>
      </div>

      {/* 展示项布局编辑器 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("popover.items", "展示项")}</div>
          <div style={{ position: "relative" }}>
            <Button
              variant="ghost"
              style={{ fontSize: 12, padding: "4px 10px", height: 28 }}
              disabled={availableTypes.length === 0}
              onClick={() => setShowAddMenu((v) => !v)}
            >
              + {t("popover.addItem", "添加项")}
            </Button>
            {showAddMenu && availableTypes.length > 0 && (
              <div className="glass-surface" style={{
                position: "absolute", top: "100%", right: 0, marginTop: 4, zIndex: 50,
                minWidth: 160, padding: 6, borderRadius: 10, display: "flex", flexDirection: "column", gap: 2,
              }}>
                {availableTypes.map((ty) => (
                  <Button
                    key={ty}
                    variant="ghost"
                    style={{ fontSize: 12, padding: "6px 10px", height: 32, justifyContent: "flex-start", textAlign: "left" }}
                    onClick={() => addItem(ty)}
                  >
                    {t(TYPE_LABELS[ty].key, TYPE_LABELS[ty].fallback)}
                  </Button>
                ))}
              </div>
            )}
          </div>
        </div>

        {rowGroups.length === 0 ? (
          <div className="text-tertiary" style={{ fontSize: 12, fontStyle: "italic", padding: "8px 0" }}>
            {t("popover.empty", "暂无展示项，点击「添加项」")}
          </div>
        ) : (
          <DndContext
            sensors={sensors}
            collisionDetection={pointerWithin}
            onDragStart={handleDragStart}
            onDragOver={handleDragOver}
            onDragEnd={handleDragEnd}
          >
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {rowGroups.map((items, row) => (
                <RowContainer
                  key={`row-${row}`}
                  row={row}
                  cols={colsForRow(row)}
                  items={items}
                  onSetCols={(c) => setRowCols(row, c)}
                >
                  {items.map((item) => (
                    <SortableCard key={item.id} item={item}>
                      <CardEditor
                        item={item}
                        t={t}
                        platforms={platforms}
                        groups={groups}
                        summary={
                          item.item_type === "cost_trend" || item.item_type === "platform_metric" || GROUP_TYPES.has(item.item_type)
                            ? trendSummary(item)
                            : previewValue(item.item_type)
                        }
                        onToggleVisible={() => toggleVisible(item.id)}
                        onRemove={() => removeItem(item.id)}
                        onUpdate={(patch) => updateItem(item.id, patch)}
                      />
                    </SortableCard>
                  ))}
                </RowContainer>
              ))}
            </div>
            <DragOverlay>
              {activeItem ? (
                <div style={{
                  padding: "8px 10px", borderRadius: 8, fontSize: 13, fontWeight: 500,
                  background: "var(--bg-floating, var(--bg-glass))", border: "1px solid var(--accent)",
                  boxShadow: "var(--shadow-lg)",
                }}>
                  {t(TYPE_LABELS[activeItem.item_type].key, TYPE_LABELS[activeItem.item_type].fallback)}
                </div>
              ) : null}
            </DragOverlay>
          </DndContext>
        )}

        <Button
          variant="ghost"
          style={{ fontSize: 11, padding: "2px 8px", height: 24, alignSelf: "flex-start", color: "var(--text-tertiary)" }}
          onClick={showLayoutHint}
        >
          {t("popover.rowHintBtn", "布局说明")}
        </Button>
      </div>

      {/* 实时预览（draft state，即改即见；与真实浮窗共用 renderGrid） */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("popover.preview", "实时预览")}</div>
        <div className="text-secondary" style={{ fontSize: 11 }}>
          {t("popover.previewHint", "下方按当前布局即时渲染浮窗外观，无需保存。")}
        </div>
        <div style={{ display: "flex", justifyContent: "center", padding: "8px 0" }}>
          {rowGroups.length === 0 ? (
            <div className="text-tertiary" style={{ fontSize: 12, fontStyle: "italic" }}>
              {t("popover.empty", "暂无展示项，点击「添加项」")}
            </div>
          ) : (
            <div className="popover-root" style={{ margin: 0 }}>
              {renderGrid(config, previewData, groups, groupDetails, t, statsCtx)}
            </div>
          )}
        </div>
      </div>

      {message && <div className="text-secondary" style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{message}</div>}
    </div>
  );
}
