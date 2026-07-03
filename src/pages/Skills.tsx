// ─── Skills 管理页 (facade) ──────────────────────────────────
// 顶层侧栏入口。统一已装列表（一条/skill，不分 agent）。
// 每行右侧展示 claude/codex 图标：在 enabled_agents 内=启用样式，否则=未启用样式，可点切换。
// 所有操作（list/enable/disable/update）全走后端 npx skills（无手动 fs）。
//
// scope 默认 Global（用户级全局 -g），可选 Project（选某项目目录）。
// npx/node 缺失 → 顶部提示条引导装 node，不阻塞整页。
//
// 拆分（arch 阶段6 S3）：state/actions 外迁 Skills/useSkillsData，JSX 分 SkillsView（主列表）+
// SkillModals（7 个 portal 弹窗），编解码/常量在 Skills/share + Skills/constants。
// 外部 import 路径（App.tsx `from "./pages/Skills"`）零 churn。

import { useSkillsData } from "./Skills/useSkillsData";
import { SkillsView } from "./Skills/SkillsView";
import { SkillModals } from "./Skills/SkillModals";

export function Skills() {
  const s = useSkillsData();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      <SkillsView s={s} />
      <SkillModals s={s} />
    </div>
  );
}
