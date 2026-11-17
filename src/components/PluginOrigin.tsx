// 「来自插件 X」标记：技能 / MCP / 子代理由 Claude 插件捆绑包分发时，在各自页面行内标出所属插件。
// 归属取自插件记录的部件清单（plugins_bundle_list）；子代理另有自身的 bundle 字段。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "./ui/badge";
import { pluginsApi } from "../services/api/plugins";

export type PluginPartKind = "skills" | "servers" | "subagents";

/** 部件名 → 插件名（按部件类别）。读取失败时为空：标记只是附加信息。 */
export function usePluginOrigins(kind: PluginPartKind): Record<string, string> {
  const [origins, setOrigins] = useState<Record<string, string>>({});
  useEffect(() => {
    pluginsApi
      .claudeList()
      .then((res) => setOrigins(Object.fromEntries(res.plugins.flatMap((p) => p[kind].map((part) => [part, p.name])))))
      .catch(() => {});
  }, [kind]);
  return origins;
}

export function PluginOriginBadge({ plugin, testId }: { plugin?: string | null; testId: string }) {
  const { t } = useTranslation();
  if (!plugin) return null;
  return (
    <Badge variant="outline" data-testid={testId}>
      {t("plugins.fromPlugin", { name: plugin })}
    </Badge>
  );
}
