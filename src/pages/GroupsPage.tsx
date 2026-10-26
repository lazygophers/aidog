// GroupsPage — 「分组」独立导航页（G3 设计稿 6：pgtop 工具栏 + 紧凑组卡 → 观测台三栏详情）。
// 薄包装：GroupsEmbedded variant="page"（数据/状态机全复用，仅列表渲染走 GroupConsoleList）。
// 成员平台编辑/复制跳平台页表单（经 onNavigate navContext，Platforms initialFilter 消费）。
import { GroupsEmbedded } from "./Groups";

export function Groups({ onNavigate }: {
  onNavigate?: (id: string, context?: { groupId?: string; groupKey?: string; platformId?: number; platformName?: string; duplicate?: boolean }) => void;
}) {
  return (
    <GroupsEmbedded
      variant="page"
      onNavigate={onNavigate}
      onEditPlatform={(p) => onNavigate?.("platforms", { platformId: p.id, platformName: p.name })}
      onDuplicatePlatform={(p) => onNavigate?.("platforms", { platformId: p.id, platformName: p.name, duplicate: true })}
    />
  );
}

export default Groups;
