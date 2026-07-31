---
title: 多源异步解析并发控制（parseInFlightRef 计数）
layer: recall
category: frontend
keywords: [parseinflight,concurrent,多源,异步,ref,计数,loading,boolean]
source: cpa-drag-import
authored-by: skein-memory
created: 1784035659
---

# 多源异步解析并发控制模式（parseInFlightRef 计数）

何时被读: 多源异步操作共享单一 loading 态（parsing/loading）时
不遵守代价: boolean 无法反映并发 → 最后一个解析完成提前关闭 loading，中间还有在跑

## 问题: boolean 无法表达「任一在解析」

源 A 完成设 false，源 B 还在跑但 UI 已显示非解析态。互斥锁过重（JS 单线程无需真锁）。

## 模式: useRef 计数（parseInFlightRef）

```typescript
const parseInFlightRef = useRef(0);

const parseAndMerge = async (path: string) => {
  parseInFlightRef.current += 1;
  setParsing(true);
  try {
    // ... 解析
  } finally {
    parseInFlightRef.current -= 1;
    if (parseInFlightRef.current === 0) setParsing(false);  // 全完才关
  }
};
```

**关键**:
- `++` 前 / `--` finally（异常也 decrement）
- `=== 0` 判全完，非 `!parsing`
- 多源 drop 可并发调（不 await），计数器正确反映在适数

## 清理

modal 关闭 `parseInFlightRef.current = 0; setParsing(false)`。

## 验收

- [ ] 快速拖 N 源 → parsing 恒 true 直到全完
- [ ] 某源失败 → 其他继续，最后完成才 false
