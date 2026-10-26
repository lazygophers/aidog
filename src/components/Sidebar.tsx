import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useApp } from "../context/AppContext";
import { ALL_LOCALES } from "../locales";
import { IconGlobe } from "./icons";
import { Button } from "@/components/ui/button";
import { makeRipple } from "../utils/motion";

// ── SVG Icons ──

const icons = {
  home: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M2.5 8 9 2.5 15.5 8" />
      <path d="M4 7v8h10V7" />
      <path d="M7 15v-4h4v4" />
    </svg>
  ),
  proxy: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 9h12M7 5l-4 4 4 4M13 5l4 4-4 4" />
    </svg>
  ),
  platforms: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="2" width="6" height="6" rx="1.5" />
      <rect x="10" y="2" width="6" height="6" rx="1.5" />
      <rect x="2" y="10" width="6" height="6" rx="1.5" />
      <rect x="10" y="10" width="6" height="6" rx="1.5" />
    </svg>
  ),
  groups: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M6 3h10M6 9h10M6 15h10M2 3h.01M2 9h.01M2 15h.01" />
    </svg>
  ),
  settings: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M7.2 1.5h3.6l.6 2.2a6 6 0 012 1.15l2.2-.55 1.8 3.12-1.8 1.48a6 6 0 010 2.4l1.8 1.48-1.8 3.12-2.2-.55a6 6 0 01-2 1.15l-.6 2.2H7.2l-.6-2.2a6 6 0 01-2-1.15l-2.2.55-1.8-3.12 1.8-1.48a6 6 0 010-2.4L.6 7.42l1.8-3.12 2.2.55a6 6 0 012-1.15z" />
      <circle cx="9" cy="9" r="2.5" />
    </svg>
  ),
  logs: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 3h12v12H3z" />
      <path d="M6 6h6M6 9h4M6 12h5" />
    </svg>
  ),
  codex: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M10 2L3 10h4l-1 6 7-8h-4l1-6z" />
    </svg>
  ),
  stats: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 15V8M7 15V5M11 15V10M15 15V3" />
    </svg>
  ),
  notifications: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 2a5 5 0 00-5 5c0 4-1.5 5-1.5 5h13S14 11 14 7a5 5 0 00-5-5z" />
      <path d="M7.5 15a1.5 1.5 0 003 0" />
    </svg>
  ),
  skills: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 1.5l2 4.5 4.5.5-3.4 3 1 4.5L9 13.5 4.9 16l1-4.5L2.5 6.5 7 6z" />
    </svg>
  ),
  mcp: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="2.5" width="5.5" height="5.5" rx="1" />
      <rect x="10.5" y="10" width="5.5" height="5.5" rx="1" />
      <path d="M7.75 5.25h2.5a2 2 0 0 1 2 2v2.75" />
    </svg>
  ),
  about: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="9" cy="9" r="7" />
      <path d="M9 8v4.5" />
      <path d="M9 5.5h.01" />
    </svg>
  ),
  chevron: (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 4.5L6 7.5L9 4.5" />
    </svg>
  ),
  sun: (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <circle cx="8" cy="8" r="3" />
      <path d="M8 1v2M8 13v2M1 8h2M13 8h2M3.05 3.05l1.41 1.41M11.54 11.54l1.41 1.41M3.05 12.95l1.41-1.41M11.54 4.46l1.41-1.41" />
    </svg>
  ),
  moon: (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14 8.5A6 6 0 018.5 2 6.5 6.5 0 1014 8.5z" />
    </svg>
  ),
};

// ── Types ──

/** 折叠子菜单项（如 settings 下的子页）。id 用 "<parent>/<sub>" 复合形式。 */
export interface NavChild {
  id: string;
  labelKey: string;
  /** 分组标题 i18n key；相邻同 group 的子项归为一节。 */
  group: string;
}

export interface NavItem {
  id: string;
  icon: string;
  labelKey: string;
  /** 所属 section（顶级分组）i18n key；相邻同 section 归为一节，节头可折叠。 */
  section?: string;
  /** 可选快捷键标识（G3 稿 6 侧栏 .k：⌘1/⌘2/⌘3，仅展示）。 */
  kbd?: string;
  /** 可选未读 badge 计数（> 0 时显示）。 */
  badge?: number;
  /** 可选折叠子菜单；存在时该项渲染为可展开分组。 */
  children?: NavChild[];
}

/** 跨页快捷跳转携带的筛选上下文（平台→日志 / 分组→统计 等）。 */
export interface NavContext {
  platformId?: number;
  platformName?: string;
  groupId?: string;
  groupKey?: string;
  model?: string;
  /** 经导航进入平台页时以「复制」（新建态）而非「编辑」打开目标平台。 */
  duplicate?: boolean;
}

interface SidebarProps {
  navItems: NavItem[];
  activeId: string;
  onNavigate: (id: string, context?: NavContext) => void;
}

// ── Dropdown Component ──

function Dropdown({
  trigger,
  children,
  open,
  onToggle,
  align = "left",
}: {
  trigger: React.ReactNode;
  children: React.ReactNode;
  open: boolean;
  onToggle: () => void;
  align?: "left" | "right";
}) {
  return (
    <div style={{ position: "relative" }}>
      <div onClick={onToggle} style={{ cursor: "pointer" }}>
        {trigger}
      </div>
      {open && (
        <>
          <div
            style={{ position: "fixed", inset: 0, zIndex: 99 }}
            onClick={onToggle}
          />
          <div
            className="glass-elevated animate-fade-in-200"
            style={{
              position: "absolute",
              bottom: "100%",
              [align]: 0,
              marginBottom: 6,
              minWidth: 180,
              padding: 6,
              zIndex: 100,
            }}
          >
            {children}
          </div>
        </>
      )}
    </div>
  );
}

function DropdownItem({
  active,
  children,
  onClick,
}: {
  active?: boolean;
  children: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <Button
      variant="ghost"
      className={active ? "ripple" : "ripple hover-lift"}
      style={{
        width: "100%",
        justifyContent: "flex-start",
        gap: 8,
        padding: "8px 10px",
        fontSize: 13,
        fontWeight: active ? 600 : 400,
        // 选中项的字与左侧竖条走 --accent / --accent-edge，不走 --primary：
        // --primary 是「填充色」，深色下是近黑（#101012），拿它当字色 = 近黑字压在
        // 近黑底上，整条菜单项看不见（用户 2026-09-23 实机报的就是这个）。
        // --accent 映射的是 token 的 accent-text（深色更亮 / 浅色更暗），是给字用的那档。
        color: active ? "var(--accent)" : "var(--text-primary)",
        background: active ? "var(--accent-subtle)" : "transparent",
        borderRadius: "var(--radius-sm)",
        height: "auto",
        boxShadow: active ? "inset 2px 0 0 var(--accent-edge), 0 0 8px color-mix(in srgb, var(--accent) 22%, transparent)" : "none",
      }}
      onClick={(e) => { makeRipple(e); onClick(); }}
    >
      <span style={{ minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {children}
      </span>
    </Button>
  );
}

// ── Main Sidebar ──

export function Sidebar({ navItems, activeId, onNavigate }: SidebarProps) {
  const { t } = useTranslation();
  const {
    locale, setLocale,
  } = useApp();
  const [langOpen, setLangOpen] = useState(false);
  // G3 骨架（票 02）：两档折叠 216px ↔ 56px 图标态，localStorage 记忆。
  const [collapsed, setCollapsed] = useState(() => {
    try { return localStorage.getItem("aidog.sidebar.collapsed") === "1"; } catch { return false; }
  });
  const toggleCollapsed = () => setCollapsed((v) => {
    try { localStorage.setItem("aidog.sidebar.collapsed", v ? "0" : "1"); } catch { /* 私有模式等 */ }
    return !v;
  });
  // 折叠子菜单展开态：用户 toggle 覆盖；未覆盖时 active 所在组自动展开。
  const [expandedNav, setExpandedNav] = useState<Record<string, boolean>>({});
  // section 折叠态：用户 toggle 覆盖；未覆盖时 active 所在 section 自动展开。
  const [collapsedSection, setCollapsedSection] = useState<Record<string, boolean>>({});

  // 顶级 section 分组：相邻同 section key 聚为一节。
  const sections: { key: string; items: NavItem[] }[] = [];
  for (const it of navItems) {
    const sk = it.section ?? "";
    const last = sections[sections.length - 1];
    if (last && last.key === sk) last.items.push(it);
    else sections.push({ key: sk, items: [it] });
  }

  return (
    <aside
      id="g3-sidebar"
      data-state={collapsed ? "collapsed" : "expanded"}
      style={{
        width: collapsed ? 56 : 216,
        minWidth: collapsed ? 56 : 216,
        height: "100%",
        display: "flex",
        flexDirection: "column",
        padding: collapsed ? "12px 8px" : "16px 12px",
        gap: 4,
        // G3：侧栏底 = bg-chrome（light #FAFBFC / dark 同页面底），右边一条 hairline
        background: "var(--bg-chrome)",
        borderInlineEnd: "1px solid var(--border)",
        transition: "width 180ms cubic-bezier(0.4, 0, 0.2, 1)",
        overflow: "hidden",
      }}
    >
      {/* Logo 行 + 折叠开关（G3：216/56 两档） */}
      <div style={{
        padding: collapsed ? "4px 0 16px" : "10px 6px 20px",
        fontSize: 15,
        fontWeight: 700,
        letterSpacing: "-0.3px",
        display: "flex",
        alignItems: "center",
        gap: 8,
      }}>
        <img
          src="/logo.svg"
          alt="logo"
          style={{ width: 26, height: 26, flexShrink: 0 }}
        />
        {!collapsed && <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", color: "var(--primary)" }}>{t("app.title")}</span>}
        <Button
          id="g3-sidebar-collapse"
          variant="ghost"
          title={collapsed ? t("sidebar.expand", "展开侧栏") : t("sidebar.collapse", "折叠侧栏")}
          onClick={toggleCollapsed}
          style={{ padding: 4, width: 24, minWidth: 0, fontSize: 11, color: "var(--text-tertiary)", height: "auto" }}
        >
          {collapsed ? "»" : "«"}
        </Button>
      </div>

      {/* Navigation — 纵向滚动: 窗口矮/分组展开多时 nav 项溢出可滚 (minHeight:0 是 flex 子项 overflow 生效关键) */}
      <nav style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column", gap: 2, overflowY: "auto" }}>
        {sections.map((sec) => {
          // section 内是否有 active 项（任一 item.id 匹配 activeId 顶级或其子页）。
          const topId = activeId.split("/")[0];
          const activeInSection = sec.items.some(it => it.id === topId || activeId.startsWith(it.id + "/"));
          const collapsed = (collapsedSection[sec.key] ?? false) && !activeInSection;
          // 无 section key（空串）= 平铺区，不渲染节头。
          const hasHeader = sec.key !== "";
          return (
            <div key={sec.key || "_"} style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              {hasHeader && !collapsed && (
                <Button
                  variant="ghost"
                  style={{
                    justifyContent: "space-between",
                    padding: "8px 10px 4px",
                    fontSize: 11,
                    fontWeight: 700,
                    color: "var(--text-tertiary)",
                    opacity: 0.7,
                    letterSpacing: "0.5px",
                    textTransform: "uppercase",
                    background: "transparent",
                    height: "auto",
                  }}
                  onClick={() => setCollapsedSection((s) => ({ ...s, [sec.key]: !collapsed }))}
                >
                  <span style={{ minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{t(sec.key)}</span>
                  <span style={{
                    opacity: 0.5,
                    transform: collapsed ? "rotate(-90deg)" : "none",
                    transition: "transform 200ms",
                    display: "inline-flex",
                  }}>{icons.chevron}</span>
                </Button>
              )}
              {(!hasHeader || !collapsed) && sec.items.map((item) => {
                const isActive = item.id === topId;
                const hasChildren = !!item.children && item.children.length > 0;
                const inThis = activeId.startsWith(item.id + "/");
                // 折叠态不展开子菜单：点击顶级项直接跳首个 child
                const expanded = !collapsed && (expandedNav[item.id] ?? (inThis ? true : false));
                return (
            <div key={item.id} style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              <Button
                variant="ghost"
                className={isActive ? "ripple" : "ripple hover-lift"}
                title={collapsed ? t(item.labelKey) : undefined}
                style={{
                  justifyContent: collapsed ? "center" : "flex-start",
                  gap: collapsed ? 0 : 9,
                  padding: collapsed ? "9px 0" : "8px 10px",
                  fontWeight: isActive ? 600 : 400,
                  // G3：选中 = accent-subtle 淡底 + 左 2px primary + 主色字（真值表 .nav .it.on）
                  color: isActive ? "var(--primary)" : "var(--text-secondary)",
                  background: isActive ? "var(--accent-subtle)" : "transparent",
                  borderInlineStart: `2px solid ${isActive ? "var(--primary)" : "transparent"}`,
                  borderRadius: "var(--radius-sm)",
                  fontSize: 12.5,
                  height: "auto",
                  position: "relative",
                }}
                onClick={(e) => {
                  makeRipple(e);
                  if (collapsed && hasChildren) { onNavigate(item.children![0].id); return; }
                  if (hasChildren) {
                    // header 点击始终 toggle 展开；仅「展开 + 未在组内」时跳首个 child。
                    // 修复：group 已展开但 activeId 不在组内时，点击应收起而非重新展开+跳转。
                    const willExpand = !expanded;
                    setExpandedNav((e) => ({ ...e, [item.id]: willExpand }));
                    if (willExpand && !inThis) {
                      onNavigate(item.children![0].id);
                    }
                  } else {
                    onNavigate(item.id);
                  }
                }}
              >
                <span style={{
                  display: "flex",
                  alignItems: "center",
                  opacity: isActive ? 1 : 0.6,
                  transition: "opacity 200ms",
                }}>
                  {(icons as Record<string, React.ReactNode>)[item.icon]}
                </span>
                {!collapsed && <span style={{ flex: 1, minWidth: 0, textAlign: "start", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{t(item.labelKey)}</span>}
                {!collapsed && item.kbd && (
                  <span style={{ fontSize: 9, opacity: 0.5, fontFamily: "var(--font-mono, monospace)", flexShrink: 0 }}>{item.kbd}</span>
                )}
                {hasChildren && !collapsed && (
                  <span style={{
                    opacity: 0.4,
                    transform: expanded ? "rotate(180deg)" : "none",
                    transition: "transform 200ms",
                    display: "inline-flex",
                  }}>{icons.chevron}</span>
                )}
                {item.badge != null && item.badge > 0 && !collapsed && (
                  <span
                    style={{
                      fontSize: 11,
                      fontWeight: 700,
                      minWidth: 16,
                      height: 16,
                      padding: "0 5px",
                      borderRadius: 999,
                      background: "var(--accent)",
                      color: "var(--accent-foreground)",
                      display: "inline-flex",
                      alignItems: "center",
                      justifyContent: "center",
                    }}
                  >
                    {item.badge > 99 ? "99+" : item.badge}
                  </span>
                )}
              </Button>
              {hasChildren && expanded && (
                <div style={{ display: "flex", flexDirection: "column", gap: 2, paddingInlineStart: 12 }}>
                  {(() => {
                    const groups: { key: string; items: NavChild[] }[] = [];
                    for (const c of item.children!) {
                      const last = groups[groups.length - 1];
                      if (last && last.key === c.group) last.items.push(c);
                      else groups.push({ key: c.group, items: [c] });
                    }
                    return groups.map((g) => (
                      <div key={g.key} style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                        <div style={{
                          fontSize: 11,
                          fontWeight: 600,
                          color: "var(--text-secondary)",
                          opacity: 0.6,
                          letterSpacing: "0.3px",
                          padding: "6px 10px 2px",
                        }}>
                          {t(g.key)}
                        </div>
                        {g.items.map((c) => {
                          const childActive = activeId === c.id;
                          return (
                            <Button
                              variant="ghost"
                              key={c.id}
                              className={childActive ? "ripple" : "ripple hover-lift"}
                              style={{
                                justifyContent: "flex-start",
                                padding: "7px 10px 7px 26px",
                                fontWeight: childActive ? 600 : 400,
                                fontSize: 12,
                                color: childActive ? "var(--primary)" : "var(--text-secondary)",
                                background: childActive ? "var(--accent-subtle)" : "transparent",
                                borderRadius: "var(--radius-sm)",
                                borderInlineStart: childActive ? "2px solid var(--primary)" : "2px solid transparent",
                                height: "auto",
                              }}
                              onClick={(e) => { makeRipple(e); onNavigate(c.id); }}
                            >
                              {t(c.labelKey)}
                            </Button>
                          );
                        })}
                      </div>
                    ));
                  })()}
                </div>
              )}
            </div>
                );
              })}
            </div>
          );
        })}
      </nav>

      {/* Bottom Controls（G3：主题切换移顶栏，这里保语言 + 用户区行） */}
      <div style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        paddingTop: 12,
        borderTop: "1px solid var(--border)",
      }}>
        {/* 用户区（G3 .nav .u）：avatar 圆 + 当前语言（折叠态只 avatar） */}
        <div id="g3-sidebar-user" style={{
          display: "flex",
          gap: 8,
          alignItems: "center",
          padding: collapsed ? "6px 0" : "6px 8px",
          fontSize: 11.5,
          color: "var(--text-secondary)",
          whiteSpace: "nowrap",
        }}>
          <span style={{
            width: 26,
            height: 26,
            borderRadius: "50%",
            background: "color-mix(in srgb, var(--primary) 14%, transparent)",
            color: "var(--primary)",
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            fontWeight: 700,
            fontSize: 11,
            flexShrink: 0,
          }}>L</span>
          {!collapsed && (
            <>
              <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{t("sidebar.localInstance", "本地实例")}</span>
              <span style={{ marginLeft: "auto", display: "inline-flex", alignItems: "center", gap: 4, flexShrink: 0 }}>
                <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--color-success)", display: "inline-block" }} />
                {t("sidebar.running", "运行中")}
              </span>
            </>
          )}
        </div>

        {/* Language Picker（折叠态隐藏，语言经展开态或设置页改） */}
        {!collapsed && (
        <Dropdown
          open={langOpen}
          onToggle={() => setLangOpen((v) => !v)}
          trigger={
            <Button variant="ghost" className="ripple hover-lift" style={{
              width: "100%",
              justifyContent: "space-between",
              fontSize: 12,
              padding: "7px 10px",
              color: "var(--text-secondary)",
              height: "auto",
            }}
            onClick={makeRipple}
            >
              <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}><IconGlobe size={14} /> {t(`lang.${locale}`)}</span>
              <span style={{ opacity: 0.4 }}>{icons.chevron}</span>
            </Button>
          }
        >
          {ALL_LOCALES.map((loc) => (
            <DropdownItem
              key={loc}
              active={loc === locale}
              onClick={() => {
                setLocale(loc);
                setLangOpen(false);
              }}
            >
              {t(`lang.${loc}`)}
            </DropdownItem>
          ))}
        </Dropdown>
        )}
      </div>
    </aside>
  );
}
