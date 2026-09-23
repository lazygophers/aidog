// 模型名渲染：表格只展示跨平台统一名；真实请求名只在详情弹窗展示。
// display_name 是人类可读名，canonical_model 是统一稳定键。

import { F } from "../../domains/shared/tokens";

export interface NameParts {
  primary: string;
  secondary: string | null;
}

/** 详情弹窗用：展示名与请求名并列。表格不调用此函数。 */
export function nameParts(displayName: string, modelId: string): NameParts {
  const name = displayName.trim();
  if (!name || name === modelId) return { primary: modelId, secondary: null };
  return { primary: name, secondary: modelId };
}

/** 表格模型名：始终展示统一 canonical_model，不展示平台请求名或平台展示名。 */
export function ModelNameCell({ canonicalModel }: { canonicalModel: string }) {
  return (
    <code style={{ fontWeight: 500, fontSize: F.small }}>{canonicalModel}</code>
  );
}
