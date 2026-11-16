import { invoke } from "../transport";

export interface Variable {
  key: string;
  value: string;
  claude: boolean;
  codex: boolean;
}

export const variablesApi = {
  saveCodex: (variables: Variable[]) =>
    invoke<string[]>("variables_codex_save", { variables }),
};
