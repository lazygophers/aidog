// ── 数值/时间格式化公共层（对应 React 版 src/utils/formatters.ts）──
// 其余格式化函数（formatNumber / formatCostUsd / formatDurationMs …）由页面票按需补进本文件，
// 禁在页面内重复定义（CLAUDE.md「数值格式化统一走 utils/formatters」）。

/// 两位补零：`pad(5) == "05"`。对应 `formatters.ts::pad`。
String pad(int n) => n.toString().padLeft(2, '0');
