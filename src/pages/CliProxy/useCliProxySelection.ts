import { useState } from "react";

// useCliProxySelection — CliProxy 页批量选择态收敛（c10）。
// 收编原主文件 7 项碎片 state（selectMode/selectedIds/3 个 batch modal 开关+各自 payload），
// 对外只暴露语义化操作（enter/exit/toggle/toggleAll/isSelected + 各 batch modal 的 open/close），
// 调用方不感知内部用 Set 还是别的结构存 selectedIds。
export function useCliProxySelection(allIds: number[]) {
  const [selectMode, setSelectMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [batchDeleteOpen, setBatchDeleteOpen] = useState(false);
  const [batchModelsOpen, setBatchModelsOpen] = useState(false);
  const [batchModelsText, setBatchModelsText] = useState("");
  const [batchQuotaOpen, setBatchQuotaOpen] = useState(false);
  const [batchQuotaType, setBatchQuotaType] = useState<"none" | "newapi">("none");

  const closeAllBatchModals = () => {
    setBatchDeleteOpen(false);
    setBatchModelsOpen(false);
    setBatchQuotaOpen(false);
  };

  const enter = () => {
    setSelectMode(true);
    setSelectedIds(new Set());
  };
  const exit = () => {
    setSelectMode(false);
    setSelectedIds(new Set());
    closeAllBatchModals();
  };
  const toggle = (id: number) => {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };
  const toggleAll = () => {
    setSelectedIds(prev => (prev.size === allIds.length ? new Set() : new Set(allIds)));
  };
  const isSelected = (id: number) => selectedIds.has(id);

  const openBatchDelete = () => setBatchDeleteOpen(true);
  const closeBatchDelete = () => setBatchDeleteOpen(false);

  const openBatchModels = () => { setBatchModelsText(""); setBatchModelsOpen(true); };
  const closeBatchModels = () => setBatchModelsOpen(false);

  const openBatchQuota = () => { setBatchQuotaType("none"); setBatchQuotaOpen(true); };
  const closeBatchQuota = () => setBatchQuotaOpen(false);

  return {
    selectMode,
    selectedIds,
    selectedCount: selectedIds.size,
    isAllSelected: allIds.length > 0 && selectedIds.size === allIds.length,
    enter,
    exit,
    toggle,
    toggleAll,
    isSelected,

    batchDeleteOpen,
    openBatchDelete,
    closeBatchDelete,

    batchModelsOpen,
    batchModelsText,
    setBatchModelsText,
    openBatchModels,
    closeBatchModels,

    batchQuotaOpen,
    batchQuotaType,
    setBatchQuotaType,
    openBatchQuota,
    closeBatchQuota,
  };
}
