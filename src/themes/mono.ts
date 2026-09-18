import { tokens } from "./tokens.generated";
import type { ThemeDefinition, ThemeMode } from "./types";

/**
 * Mono 主题 —— 色值来源是跨栈 token 表（票 13）。
 *
 * 真值源：`design/tokens/tokens.json`（同一份表也生成 Flutter 侧的 `tokens.dart`）。
 * 改色一律改那份 JSON 再跑 `yarn tokens`；本文件只做「token 语义名 → shadcn 语义变量」的映射，
 * 不放任何字面色值。`yarn check:tokens` 拦住「改了 JSON 忘了重新生成」。
 *
 * 本文件只换颜色的来源，不改版式：radius / blur / transition / shadow 三档仍是原值
 * （token 表里的 radius 是 A′ 原型的版式，套到现仓等于改版，不在票 13 范围）。
 *
 * 映射里几个不是一对一的地方，理由写在各行。token 表缺项见 `.scratch/flutter-frontend/research/13-tokens.md`。
 */
function vars(mode: ThemeMode): Record<string, string> {
  const c = tokens[mode];
  // accent 的 rgb 三元组：--app-bg-overlay 的光晕要跟着主色走，token 表没有单独的
  // overlay 色（已回报票 10），这里从 accent 派生而不是另写一个字面色。
  const accentRgb = mode === "dark" ? "94, 106, 210" : "78, 89, 196";
  return {
    // ── 结构（非颜色，票 13 不动）──
    "--radius-sm": "8px",
    "--radius-md": "12px",
    "--radius-lg": "16px",
    "--radius-xl": "24px",
    "--glass-blur": "20px",
    "--glass-saturate": "1.4",
    "--glass-border": "1px solid var(--glass-edge)",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
    // token 表只有 shadow-tile / shadow-float 两档，现仓要 sm/md/lg 三档 —— 缺项已回报票 10，
    // 在补进真值源之前保持原值不动（不现编一套三档）。
    "--shadow-sm":
      mode === "dark"
        ? "0 1px 3px rgba(0, 0, 0, 0.4)"
        : "0 1px 3px rgba(28, 25, 23, 0.04), 0 1px 2px rgba(28, 25, 23, 0.02)",
    "--shadow-md": mode === "dark" ? "0 4px 20px rgba(0, 0, 0, 0.5)" : "0 4px 20px rgba(28, 25, 23, 0.06)",
    "--shadow-lg": mode === "dark" ? "0 8px 32px rgba(0, 0, 0, 0.6)" : "0 8px 32px rgba(28, 25, 23, 0.08)",
    "--shadow-color": mode === "dark" ? "0, 0, 0" : "28, 25, 23",
    // 背景光晕：形状（角度/半径/透明度）保持原样，只把颜色换成主色。
    "--app-bg-overlay":
      mode === "dark"
        ? `radial-gradient(80% 50% at 50% -12%, rgba(${accentRgb}, 0.10), transparent 60%), ` +
          `radial-gradient(56% 42% at 10% 20%, rgba(${accentRgb}, 0.06), transparent 58%)`
        : `radial-gradient(72% 52% at 50% -10%, rgba(${accentRgb}, 0.10), transparent 62%), ` +
          `radial-gradient(52% 44% at 92% 8%, rgba(${accentRgb}, 0.08), transparent 60%), ` +
          `radial-gradient(60% 50% at 6% 100%, rgba(${accentRgb}, 0.06), transparent 64%)`,

    // ── 色（全部来自 token 表）──
    "--background": c.bg,
    "--foreground": c.fg,
    "--card": c.surface,
    "--card-foreground": c.fg,
    "--popover": c.surface,
    "--popover-foreground": c.fg,
    "--primary": c.accent,
    // token 表没有「主色底上的字色」。深色下用 fg，浅色下用 surface(#FFF)：两者都是表里已有的值，
    // 与 accent 的对比度均 > 4.5:1。缺项已回报票 10（建议补 accent-fg）。
    "--primary-foreground": mode === "dark" ? c.fg : c.surface,
    "--secondary": c["surface-2"],
    "--secondary-foreground": c.fg,
    "--muted": c["surface-2"],
    // globals.css 把 --text-tertiary 直接别名到 --muted-foreground，三级文字梯度的最底一级
    // 对应 token 的 fg-3；--text-secondary 由 globals.css 用 fg×45% 混出来，落在 fg-2 附近。
    "--muted-foreground": c["fg-3"],
    // --accent 在本仓是「看得见的主色」：全库 110 处里绝大多数拿它当图标色 / 链接色 / 边框色
    // （SectionIcon、全选/反选、GroupIcon、协议品牌色回落…），只有 shadcn 的 hover 态拿它当底色。
    // 所以对应 token 的 accent-text（深色更亮 / 浅色更暗），不是「格子 hover」的 surface-2 ——
    // 映射成 surface-2 会让那 110 处图标与链接在深色下直接消失（已实测，见 13-tokens.md）。
    "--accent": c["accent-text"],
    // 配套字色：accent-text 深色下是亮紫 → 配深字；浅色下是深紫 → 配白字。与旧金底 idiom 同构。
    "--accent-foreground": mode === "dark" ? c.bg : c.surface,
    "--destructive": c.bad,
    "--destructive-foreground": mode === "dark" ? c.fg : c.surface,
    "--border": c.line,
    "--input": c["line-strong"],
    // 焦点环 = 主色带透明度，token 表里就是 live-edge（活着的格子的边）。
    "--ring": c["live-edge"],
  };
}

export const mono: ThemeDefinition = {
  light: vars("light"),
  dark: vars("dark"),
};
