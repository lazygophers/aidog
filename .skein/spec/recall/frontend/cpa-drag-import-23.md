---
title: 多源批量导入 rowId 唯一性模式（baseIdx 偏移）
layer: recall
category: frontend
keywords: [rowid,unique,多源,import,baseidx,偏移,batch,react key]
source: cpa-drag-import
authored-by: skein-memory
created: 1784035659
---

# 多源批量导入 rowId 唯一性模式

何时被读: 多源批量导入/聚合，每条记录需全局唯一 rowId 时
不遵守代价: 跨源撞 id → React key 冲突 → 列表渲染异常 / 选中状态错乱

## 问题: 跨源 rowId 撞 id

每源 rowId 从 `${0}::` 起递增，不同源同索引条目撞 id。

## 模式: baseIdx 全局偏移（orderLenRef）

```typescript
const orderLenRef = useRef(0);

const parseAndMerge = async (path: string) => {
  const baseIdx = orderLenRef.current;  // 处理前取
  const enriched = plats.map((p, idx) => {
    const rowId = `${baseIdx + idx}::${p.name}::${p.base_url}`;
    // ...
  });
  // 增量合并（非覆盖）
  setRows(prev => ({ ...prev, ...addRows }));
  setOrder(prev => [...prev, ...newIds]);
  orderLenRef.current = baseIdx + enriched.length;  // commit 后 bump
};
```

**关键**:
- baseIdx 处理前取，commit 后 bump（防并发读 stale）
- 增量合并用对象扩展 `...prev, ...addRows`，非替换

## 清理

modal 关闭重置 `orderLenRef.current = 0`，下次打开从 0 起。

## 验收

- [ ] 多源 drop → 所有条目 rowId 唯一
- [ ] modal 重开 → orderLenRef 清零
