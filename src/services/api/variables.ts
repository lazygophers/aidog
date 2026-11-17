import { invoke } from "../transport";

export interface Variable {
  key: string;
  value: string;
}

export const variablesApi = {
  /** 独立保存：同名附加并覆盖 Claude Code 与 Codex，两边同时生效；空值 = 撤销。 */
  save: (variables: Variable[]) => invoke<void>("variables_save", { variables }),
};
