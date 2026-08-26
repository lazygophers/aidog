// 模型名渲染的单一规则：展示名与真实请求名同屏并列，两者是同一串时只渲染一次。
//
// 回落（display_name 缺省 → model_id）已经发生在后端读取层（T10），这里**不写第二份回落分支**：
// 只判「拿到的展示名和请求名是不是同一串」，是就退化成单行，避免同一个 model_id 上下重复两遍。

import { F } from "../../domains/shared/tokens";

export interface NameParts {
  /** 行首主标题。 */
  primary: string;
  /** 次要行（真实请求名）；与主标题同串时为 null —— 不渲染空节点。 */
  secondary: string | null;
}

/**
 * @param displayName 读取层给出的展示名（缺省时后端已回落成 modelId）
 * @param modelId     真实上游请求名
 */
export function nameParts(displayName: string, modelId: string): NameParts {
  const name = (displayName ?? "").trim();
  // name 为空只可能来自异常数据；退化成单行请求名而不是渲染空白单元格。
  if (!name || name === modelId) return { primary: modelId, secondary: null };
  return { primary: name, secondary: modelId };
}

/** 列表单元格：展示名大字 + 请求名小号 code；两者同串时只有一行 code。 */
export function ModelNameCell({ displayName, modelId }: { displayName: string; modelId: string }) {
  const { primary, secondary } = nameParts(displayName, modelId);
  return (
    <div style={{ display: "flex", flexDirection: "column" }}>
      {secondary === null ? (
        <code style={{ fontWeight: 500, fontSize: F.small }}>{primary}</code>
      ) : (
        <>
          <span style={{ fontWeight: 500, fontSize: F.small }}>{primary}</span>
          <code className="text-tertiary" style={{ fontSize: 11 }}>{secondary}</code>
        </>
      )}
    </div>
  );
}
