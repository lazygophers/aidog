---
title: 脏数据入库时归一 — 浮点 hour 拆分为整数 hour+minute
layer: recall
category: frontend
keywords: [脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性]
source: time-models-timezone task prd.md 边界
authored-by: skein-spec
created: 1753805040
status: active
related: [time-zone-minute-arithmetic]
updated: 1753805040
---

## 触发场景

系统升级或跨版本迁移中，存量数据可能包含不符合当前数据格式的脏数据。例如，旧版本按整小时换算时产生 `start_hour: 8.5`（半时区换算残留），新版本期望整数。

## 陷阱：后端 migration 改 serde 类型成本高，数据污染持久

> 旧版本：`peak_hours` 整小时换算，半时区用户产生 `start_hour: 8.5` 写入 JSON。后端声明 `start_hour: i32`，JSON 反序列化失败 → 静默丢弃窗口。用户配置无声失效。
>
> 修复选项：
> - ❌ 改后端存储类型为 f64 —— 一个不再产生的格式永久兼容，代价不成比例
> - ✅ 前端 parse 层吸收 —— 加载时拆分，用户下次保存自动正规化

## 正解：前端读取路径归一（关键）

### MUST 单点归一（parse 层）

```ts
/** 存量非整数 start_hour/end_hour（半时区旧逻辑产物，如 8.5）拆为 hour+minute。
 *  整数值原样不动。完全无副作用：整数返回同对象；非整数生成新对象。 */
export function normalizeWindow(w: PeakWindow): PeakWindow {
  let result = w;
  if (!Number.isInteger(w.start_hour)) {
    const { hour, minute } = splitFraction(w.start_hour, w.start_minute);
    result = { ...result, start_hour: hour, start_minute: minute };
  }
  if (!Number.isInteger(w.end_hour)) {
    const { hour, minute } = splitFraction(w.end_hour, w.end_minute);
    result = { ...result, end_hour: hour, end_minute: minute };
  }
  return result;
}

/** 纯函数：浮点时刻拆为整数 hour+minute。 */
function splitFraction(h: number, existingMinute: number | undefined): { hour: number; minute: number } {
  const hour = Math.floor(h);
  const extraMinutes = Math.round((h - hour) * 60);
  // 叠加已有 minute，超过 59 自动借位给 hour
  return shiftClock(hour, (existingMinute ?? 0) + extraMinutes, 0);
}
```

### MUST 调用路径（两处 parse 出口）

```ts
// src/services/api/platforms.ts:204-216
export function parsePlatformPeakHours(raw: unknown): PeakWindow[] {
  // ... schema 校验 ...
  return windows.map(normalizeWindow);  // ← 追加归一
}

// src/services/api/platforms.ts:272
export function parsePlatformTimeModels(raw: unknown): TimeModelRule[] {
  // ... 循环处理 rule ...
  return rules.map(r => ({
    ...r,
    windows: r.windows.map(normalizeWindow),  // ← 追加归一
  }));
}
```

### MUST 单测覆盖（脏数据拆分规则）

```ts
describe("normalizeWindow", () => {
  it("整数 hour 原样不变", () => {
    const w = { start_hour: 8, end_hour: 20, multiplier: 1.5 } as PeakWindow;
    expect(normalizeWindow(w)).toEqual(w);
  });

  it("8.0 视为整数不变（Number.isInteger(8.0) === true）", () => {
    // JSON 中 8.0 在 JS 加载后 === 8，Number.isInteger 返 true
    const w = { start_hour: 8.0, end_hour: 20, multiplier: 1.5 } as PeakWindow;
    expect(normalizeWindow(w)).toEqual(w);  // ✅ 不误拆
  });

  it("8.5 拆分为 8:30", () => {
    const w = { start_hour: 8.5, end_hour: 20, multiplier: 1.5 } as PeakWindow;
    const result = normalizeWindow(w);
    expect(result.start_hour).toBe(8);
    expect(result.start_minute).toBe(30);
    expect(result.end_hour).toBe(20);
  });

  it("已有 start_minute 时叠加进位", () => {
    // 8.5 = 8:30，加已有 40 分钟 = 9:10（借位）
    const w = { start_hour: 8.5, start_minute: 40, end_hour: 20, multiplier: 1.5 } as PeakWindow;
    const result = normalizeWindow(w);
    expect(result.start_hour).toBe(9);
    expect(result.start_minute).toBe(10);
  });

  it("start 与 end 各自独立归一", () => {
    const w = { start_hour: 8.5, end_hour: 20.25, multiplier: 1.5 } as PeakWindow;
    const result = normalizeWindow(w);
    expect(result.start_hour).toBe(8);
    expect(result.start_minute).toBe(30);
    expect(result.end_hour).toBe(20);
    expect(result.end_minute).toBe(15);
  });
});
```

### Number.isInteger 判据（精确）

```ts
// ✅ 正确：JavaScript 中 8.0 === 8，Number.isInteger(8.0) === true
Number.isInteger(8.0)      // true → 不走归一
Number.isInteger(8.5)      // false → 走归一，拆为 8:30
Number.isInteger(8.00001)  // false → 拆为 8:00 或 8:01

// ❌ 禁用其他判据（都不可靠）
h % 1 === 0        // 浮点舍入，8.00000001 失败
h === Math.floor(h) // 同上
```

## 反例 / 常见错误

| 错误                          | 为什么错                                        | 正确做法                                      |
| ----------------------------- | ----------------------------------------------- | ----------------------------------------- |
| 后端改 serde 类型为 f64        | 永久兼容一个不再产生的格式，代价不成比例        | 前端 parse 层吸收，用户保存时自动正规化 |
| 直接 Math.floor(h)，丢失分钟 | 8.5 → 8，漏掉 30 分钟，显示错误               | splitFraction 拆分 hour+minute 再叠加 |
| 判据用 `h % 1 === 0`         | 浮点舍入误差（8.0000001 判失败），覆盖不全    | 用 Number.isInteger（精确判整数）   |
| 忘记叠加已有 start_minute   | 8.5 + 已有 40m → 应是 9:10，但只变成 8:30    | splitFraction 调 shiftClock 自动借位 |
| parse 层不覆盖 time_models   | 只改 peak_hours，time_models 仍有脏数据       | 两处 parse 出口同时 map(normalizeWindow) |

## 落地 checklist

```bash
# 1. 验证 normalizeWindow 实现
grep -A15 "export function normalizeWindow" src/utils/peakHours.ts

# 2. 验证两处 parse 调用
grep -rn "map(normalizeWindow)" src/services/api/platforms.ts

# 3. 验证单测覆盖脏数据场景
grep -n "8.5\|8.0\|Number.isInteger" src/utils/peakHours.test.ts

# 4. 验证不依赖后端改动
grep -n "f64\|Float" src-tauri/crates/aidog_core/gateway/time_models.rs | wc -l  # 应该 0
```

## 验证场景

- 升级前存量：`{ start_hour: 8.5, end_hour: 20 }`（脏数据）
- 加载时：normalizeWindow → `{ start_hour: 8, start_minute: 30, end_hour: 20 }`（正规化）
- 用户修改时：自然写回整数，下次加载已正规化
- 零成本迁移：无 DB migration，无后端改动

## 适用

- 版本升级中的数据兼容性问题
- 存量脏数据前端吸收而非后端永久兼容

## 关联

[[time-zone-minute-arithmetic]] (时区换算硬约束) · [[module-load-time-constant-test-rule]] (单测规则)

## 案例

- time-models-timezone task (commit d5b00753) — normalizeWindow 及单测 + s1 完成
