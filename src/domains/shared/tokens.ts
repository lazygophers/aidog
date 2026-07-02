// 共享设计 token —— 消除 Logs/Stats/Groups/Home/PricingTab 的本地 F/S 重复定义
// ponytail: 统一 body=15 / title=20（Home 原 body=14、PricingTab 原 title=15/body=14 视觉差异可忽略）

export const F = { title: 20, kpi: 30, label: 15, body: 15, hint: 13, small: 12 } as const;

export const S = { gap: 18, pad: 28, inputPad: "10px 14px", btnPad: "8px 18px", btnIcon: 34 } as const;
