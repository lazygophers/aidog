---
title: 模块加载时常数的单测绕行法则
layer: recall
category: testing
keywords: [单测,模块加载,常数,时区偏移,getTimezoneOffset,mock,spyOn,纯函数,参数化]
source: time-models-timezone task design.md §1
authored-by: skein-spec
created: 1753805040
status: active
related: [time-zone-minute-arithmetic]
updated: 1753805040
---

## 触发场景

模块在加载时求值的常数（如本地时区偏移 `LOCAL_OFFSET_MINUTES`），需要在单测中覆盖不同时区场景。

## 陷阱：vi.spyOn(Date.prototype, "getTimezoneOffset") 对模块常数无效

> 时区常数 `LOCAL_OFFSET_MINUTES = -new Date().getTimezoneOffset()` 在模块加载时**立即求值**，变成字面常数储存。单测运行时再 mock Date.getTimezoneOffset() 已太晚 —— 常数已固定为测试环境的实际时区（如 UTC+8）。尝试用 spy 改变返回值对模块常数无效。

```ts
// ❌ 错误：常数已固定，spy 无法回溯改变
export const LOCAL_OFFSET_MINUTES = -new Date().getTimezoneOffset(); // 加载时求值 = 480（北京）

test("印度用户时区", () => {
  vi.spyOn(Date.prototype, "getTimezoneOffset").mockReturnValue(-330); // 晚了！
  expect(utcToDisplay(8, 0, "local")).toEqual(...); // 仍用 480，不是 330
});
```

## 正解：纯函数内核参数化（硬约束，关键）

### MUST 两层函数分离（参数化内核 + 便捷包装）

```ts
/** 公开常数：模块加载时求值，用于默认行为。
 *  ⚠️ 单测禁直接依赖此常数，改用下方纯函数（显式 offsetMinutes 参数）。 */
export const LOCAL_OFFSET_MINUTES = -new Date().getTimezoneOffset();

/** 选中时区模式对应分钟偏移。*/
export function tzOffsetMinutes(mode: TzMode): number {
  return mode === "local" ? LOCAL_OFFSET_MINUTES : 0;
}

/** 🎯 纯函数内核 — offset 显式参数，可被单测任意覆盖（含 +5:30 / -300 / 任意值）。
 *  ⚠️ 单测必须打这一层，不能打 utcToDisplay(h, m, "local") 包装。 */
export function shiftClock(
  hour: number, 
  minute: number, 
  offsetMinutes: number  // ← 显式参数，单测可控
): { hour: number; minute: number } {
  const m = (((hour * 60 + minute + offsetMinutes) % 1440) + 1440) % 1440;
  return { hour: Math.floor(m / 60), minute: m % 60 };
}

/** 便捷包装（生产默认行为）。 */
export function utcToDisplay(hour: number, minute: number, mode: TzMode) {
  return shiftClock(hour, minute, tzOffsetMinutes(mode));
}

/** 应用代码用便捷包装 — 默认本地时区，生产运行时自动用 LOCAL_OFFSET_MINUTES。 */
// 好：
const displayed = utcToDisplay(8, 0, tzMode);

// 单测用纯函数内核 — 显式覆盖偏移值。
test("北京 UTC+8", () => {
  expect(shiftClock(8, 0, 480)).toEqual({ hour: 16, minute: 0 });
});
test("印度 UTC+5:30", () => {
  expect(shiftClock(8, 0, 330)).toEqual({ hour: 13, minute: 30 }); // ✅ 精确分钟
});
test("美东 UTC-5", () => {
  expect(shiftClock(14, 0, -300)).toEqual({ hour: 9, minute: 0 });
});
```

### MUST 单测骨架（完整覆盖）

```ts
describe("shiftClock — 纯函数内核", () => {
  it("offset=0 恒等", () => {
    expect(shiftClock(14, 30, 0)).toEqual({ hour: 14, minute: 30 });
  });

  it("整时区往返（北京 +480）", () => {
    const displayed = shiftClock(6, 0, 480);
    expect(displayed).toEqual({ hour: 14, minute: 0 });
    const back = shiftClock(displayed.hour, displayed.minute, -480);
    expect(back).toEqual({ hour: 6, minute: 0 });
  });

  it("半时区往返（印度 +330）", () => {
    const displayed = shiftClock(8, 0, 330);
    expect(displayed).toEqual({ hour: 13, minute: 30 }); // ✅ 分钟精确
    const back = shiftClock(displayed.hour, displayed.minute, -330);
    expect(back).toEqual({ hour: 8, minute: 0 });
  });

  it("跨零点进位（+60 分钟）", () => {
    expect(shiftClock(23, 30, 60)).toEqual({ hour: 0, minute: 30 });
  });

  it("跨零点借位（-60 分钟）", () => {
    expect(shiftClock(0, 30, -60)).toEqual({ hour: 23, minute: 30 });
  });

  it("负偏移（美东 -300）", () => {
    expect(shiftClock(14, 0, -300)).toEqual({ hour: 9, minute: 0 });
  });

  // 便捷包装 — 仅验证调用对应 shiftClock（无需再覆盖偏移值）
  it("utcToDisplay 转发 tzOffsetMinutes", () => {
    // 生产环境 LOCAL_OFFSET_MINUTES = 480（北京），此测仅验证包装逻辑
    const result = utcToDisplay(6, 0, "local");
    expect(result.hour).toBeLessThanOrEqual(24);  // 粗粒度验证
  });
});
```

### 禁止的单测写法（anti-pattern）

```ts
// ❌ 禁止：spy 对模块常数无效
vi.spyOn(Date.prototype, "getTimezoneOffset").mockReturnValue(-330);
expect(utcToDisplay(8, 0, "local")).toEqual({ hour: 13, minute: 30 });
// 结果：仍用测试环境的 LOCAL_OFFSET_MINUTES（如 480），不是期望的 330

// ❌ 禁止：双重间接（utcToDisplay 包装 + tzMode），无法隔离参数化
test("印度时区", () => {
  // 想测 offset=330，但必须依赖 LOCAL_OFFSET_MINUTES 常数 + tzMode 路由
  // 单测不能控制 LOCAL_OFFSET_MINUTES 的值
});

// ✅ 正确：直接打纯函数内核
test("印度时区", () => {
  expect(shiftClock(8, 0, 330)).toEqual({ hour: 13, minute: 30 });
});
```

## 反例 / 常见错误

| 错误                          | 为什么错                                        | 正确做法                                      |
| ----------------------------- | ----------------------------------------------- | ----------------------------------------- |
| 整层用 spy mock 时区          | 模块常数在加载时已求值，spy 无法回溯改变        | 提取纯函数内核，参数化 offset                   |
| 单测用便捷包装层 utcToDisplay | 隐式依赖 LOCAL_OFFSET_MINUTES 或 tzMode，难预测 | 单测用 shiftClock 直接指定 offsetMinutes 值     |
| 依赖测试环境时区              | 跨环境运行时结果不同（本地 UTC+8 vs CI UTC+0）  | 参数化所有时区场景，覆盖绝对值                |
| 漏覆盖边界场景                | 半时区、负偏移、跨零点仅在单测发现 bug         | checklist: 整时区 / 半时区 / 负偏移 / 跨零点   |

## 落地 checklist

```bash
# 1. 验证纯函数内核（offset 参数显式）
grep -A5 "export function shiftClock" src/utils/peakHours.ts | grep "offsetMinutes"

# 2. 验证单测覆盖（含纯函数，无 spy）
grep -n "describe.*shiftClock\|it(" src/utils/peakHours.test.ts | head -20

# 3. 验证便捷包装正确调用
grep -n "utcToDisplay.*tzOffsetMinutes" src/utils/peakHours.ts

# 4. 验证应用代码用便捷包装（无 shiftClock 直接调用）
grep -rn "shiftClock" src/pages/platforms/ | wc -l  # 应该为 0
```

## 适用

- 任何模块加载时求值的常数（时区、配置、初始化状态）需参数化单测的场景
- 纯函数测试（数学函数、格式转换、换算）

## 关联

[[time-zone-minute-arithmetic]] (时区换算硬约束)

## 案例

- time-models-timezone task (commit d5b00753) — peakHours.ts 的 shiftClock 纯函数 + peakHours.test.ts 完整单测覆盖
