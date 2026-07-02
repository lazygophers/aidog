// ─── Design tokens (settings/editors domain) ──────────────
// ponytail: editors 域 F/S 含 sectionGap/row/labelW 等专属字段，非 shared tokens 超集。
// 与 src/domains/shared/tokens.ts 语义不同（shared S 无 sectionGap/row/labelW），独立保留。
// 见 arch-redesign D2/D8 决策：editors F/S 因命名空间不同留 settings 域。

export const F = {
  title: 20,        // section heading
  label: 15,        // field label
  body: 15,         // input / button / general text
  hint: 13,         // secondary / key-in-parens / description
  small: 12,        // arrow icon / error
} as const;

export const S = {
  sectionGap: 20,   // between section cards
  gap: 18,          // between fields within a section
  row: 12,          // kv row gap
  pad: 28,          // card padding
  inputPad: "10px 14px",
  btnPad: "8px 18px",
  btnIcon: 34,      // icon button size
  labelW: 200,      // unified label column width (FieldLabel / EnvVarRow / attribution)
} as const;
