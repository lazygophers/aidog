/// 侧栏 = 格子盘的**第 0 列**，不是一块独立玻璃面板。
///
/// 它没有自己的「面板」外观：没有玻璃底、没有独立圆角、没有阴影，只靠一条与格子同色的
/// hairline 与内容区分开（`A2-bento-sidebar.html` 的 `.rail`）。现仓 `Sidebar.tsx:247`
/// 的 `glass glass-highlight` 正是**不能**搬过来的那一条 —— 搬了母题当场碎。
///
/// 沿用现仓的信息结构（5 个 section / 可折叠 / 13 个设置子页分 5 组 / badge / 底部主题与
/// 语言切换），不沿用它的实现（自写 Dropdown、11 个内联 SVG）。
/// **没有 44px 横向 chip 轨**：两套导航只能活一套，而 chip 轨装不下设置的 13 个子页。
library;

import 'package:flutter/material.dart';

import 'nav.dart';
import 'theme.dart';

/// 图标走 Material 内置字体（不再抄 11 个内联 SVG）。key 与 [NavItem.icon] 对齐。
const Map<String, IconData> kNavIcons = {
  'home': Icons.home_outlined,
  'platforms': Icons.grid_view_outlined,
  'groups': Icons.workspaces_outlined,
  'stats': Icons.bar_chart_outlined,
  'logs': Icons.list_alt_outlined,
  'notifications': Icons.notifications_none,
  'skills': Icons.auto_awesome_outlined,
  'mcp': Icons.extension_outlined,
  'settings': Icons.settings_outlined,
  'about': Icons.info_outline,
};

class Rail extends StatefulWidget {
  const Rail({
    super.key,
    required this.items,
    required this.activeId,
    required this.onNavigate,
    required this.collapsed,
    required this.onToggleCollapsed,
    required this.isDark,
    required this.onToggleTheme,
    required this.localeLabel,
    required this.onPickLocale,
    this.t = _identity,
  });

  final List<NavItem> items;
  final String activeId;
  final void Function(String id) onNavigate;
  final bool collapsed;
  final VoidCallback onToggleCollapsed;
  final bool isDark;
  final VoidCallback onToggleTheme;
  final String localeLabel;
  final VoidCallback onPickLocale;

  /// i18n 取词。票 I03 接真的 locale 表，在那之前默认原样返回 key。
  final String Function(String key) t;

  static String _identity(String k) => k;

  @override
  State<Rail> createState() => _RailState();
}

class _RailState extends State<Rail> {
  /// 子菜单展开态：用户 toggle 覆盖；未覆盖时 active 所在组自动展开（同 Sidebar.tsx:322）。
  final Map<String, bool> _expanded = {};

  String get _topId => widget.activeId.split('/').first;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final mini = widget.collapsed;
    return AnimatedContainer(
      duration: AidogMotion.base,
      curve: AidogMotion.easeStandard,
      width: mini ? AidogLayout.railWCollapsed : AidogLayout.railW,
      padding: const EdgeInsets.all(AidogSpace.smd),
      decoration: BoxDecoration(
        color: t.c.bgChrome,
        // 方向无关（规则 6）：BorderDirectional.end 在 RTL 下自动落到另一侧。
        border: BorderDirectional(end: BorderSide(color: t.c.line)),
      ),
      // 内容按**终态**宽度排版，多出来的由 ClipRect 盖掉。
      // 不这么做的话，AnimatedContainer 收宽的中途子项还在按展开态排版，
      // RenderFlex 会在 debug 下抛 overflow 断言 —— ClipRect 只管绘制裁剪，管不住断言。
      child: ClipRect(
        child: OverflowBox(
          alignment: AlignmentDirectional.topStart,
          minWidth: _contentW(mini),
          maxWidth: _contentW(mini),
          child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(
              child: SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: _sections(t, mini),
                ),
              ),
            ),
            _foot(t, mini),
          ],
          ),
        ),
      ),
    );
  }

  /// 内容区的终态宽度 = 轨宽减两侧 padding。
  static double _contentW(bool mini) =>
      (mini ? AidogLayout.railWCollapsed : AidogLayout.railW) -
      AidogSpace.smd * 2;

  List<Widget> _sections(AidogTheme t, bool mini) {
    final sections = groupAdjacent(widget.items, (NavItem i) => i.section ?? '');
    final out = <Widget>[];
    for (final sec in sections) {
      // 空 section key = 平铺区，不渲染节头（与 Sidebar.tsx:289 同规则）。
      if (sec.key.isNotEmpty && !mini) {
        out.add(_sectionHead(t, sec.key));
      }
      // 节不再折叠，整节的项一律渲染。
      for (final item in sec.items) {
        out.addAll(_navItem(t, item, mini));
      }
    }
    return out;
  }

  /// 节头是**纯标题**，不可点、不可折叠（用户 2026-09-21 定：「概览、集成这样的，不允许折叠」）。
  /// 这是对 `Sidebar.tsx:287` 的有意偏离——那边节头点一下会把整节收起来。
  Widget _sectionHead(AidogTheme t, String key) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AidogSpace.smd,
        AidogSpace.smd,
        AidogSpace.smd,
        AidogSpace.sxs,
      ),
      child: Text(
        widget.t(key).toUpperCase(),
        style: AidogType.micro.copyWith(color: t.c.fg3),
        overflow: TextOverflow.ellipsis,
      ),
    );
  }

  List<Widget> _navItem(AidogTheme t, NavItem item, bool mini) {
    final isActive = item.id == _topId;
    final inThis = widget.activeId.startsWith('${item.id}/');
    final expanded = _expanded[item.id] ?? inThis;
    final label = widget.t(item.labelKey);

    final row = _NavButton(
      // 活跃项 = 活着的格子：live-fill + live-edge + live-halo（浅色下 halo 为 none）。
      active: isActive,
      mini: mini,
      icon: kNavIcons[item.icon] ?? Icons.circle_outlined,
      label: label,
      badge: item.badge,
      chevron: item.hasChildren && !mini ? expanded : null,
      onTap: () {
        if (item.hasChildren) {
          // 点节头始终 toggle；仅「展开 + 不在组内」时跳首个子页（同 Sidebar.tsx:350-357）。
          final willExpand = !expanded;
          setState(() => _expanded[item.id] = willExpand);
          if (willExpand && !inThis) widget.onNavigate(item.children.first.id);
        } else {
          widget.onNavigate(item.id);
        }
      },
    );

    if (!item.hasChildren || !expanded || mini) return [row];

    // 13 个设置子页分 5 组 —— 横向 chip 轨装不下的正是这一块。
    final groups = groupAdjacent(item.children, (NavChild c) => c.group);
    return [
      row,
      Padding(
        padding: const EdgeInsetsDirectional.only(
          start: AidogSpace.sxl,
          top: 2,
          bottom: AidogSpace.sxs,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final g in groups) ...[
              Padding(
                padding: const EdgeInsets.fromLTRB(
                  AidogSpace.ssm,
                  AidogSpace.ssm,
                  AidogSpace.ssm,
                  2,
                ),
                child: Text(
                  widget.t(g.key).toUpperCase(),
                  style: AidogType.micro.copyWith(color: t.c.fg3),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              for (final c in g.items)
                _Tappable(
                  onTap: () => widget.onNavigate(c.id),
                  padding: const EdgeInsets.symmetric(
                    vertical: 5,
                    horizontal: AidogSpace.ssm,
                  ),
                  radius: AidogRadius.sm,
                  background: widget.activeId == c.id ? t.c.liveFill : null,
                  child: Text(
                    widget.t(c.labelKey),
                    style: AidogType.label.copyWith(
                      color: widget.activeId == c.id ? t.c.fg : t.c.fg2,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
          ],
        ),
      ),
    ];
  }

  Widget _foot(AidogTheme t, bool mini) {
    return Container(
      padding: const EdgeInsets.only(top: AidogSpace.smd),
      decoration: BoxDecoration(border: Border(top: BorderSide(color: t.c.line))),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _FootButton(
            key: const Key('rail-theme-toggle'),
            mini: mini,
            icon: widget.isDark ? Icons.dark_mode_outlined : Icons.light_mode_outlined,
            label: widget.t(widget.isDark ? 'theme.dark' : 'theme.light'),
            onTap: widget.onToggleTheme,
          ),
          _FootButton(
            key: const Key('rail-locale'),
            mini: mini,
            icon: Icons.language_outlined,
            label: widget.localeLabel,
            onTap: widget.onPickLocale,
          ),
          _FootButton(
            key: const Key('rail-collapse'),
            mini: mini,
            icon: mini ? Icons.chevron_right : Icons.chevron_left,
            label: widget.t('nav.collapse'),
            onTap: widget.onToggleCollapsed,
          ),
        ],
      ),
    );
  }
}

class _NavButton extends StatelessWidget {
  const _NavButton({
    required this.active,
    required this.mini,
    required this.icon,
    required this.label,
    required this.onTap,
    this.badge,
    this.chevron,
  });

  final bool active;
  final bool mini;
  final IconData icon;
  final String label;
  final VoidCallback onTap;
  final int? badge;
  final bool? chevron;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: _Tappable(
        onTap: onTap,
        radius: AidogRadius.sm,
        background: active ? t.c.liveFill : null,
        border: active ? t.c.liveEdge : null,
        // 深色下发光，浅色下 token 把 halo 置 none、由 fill + edge 接替。
        shadow: active ? t.liveHalo : null,
        padding: EdgeInsets.symmetric(
          vertical: 7,
          horizontal: mini ? 0 : AidogSpace.smd,
        ),
        child: Row(
          mainAxisAlignment:
              mini ? MainAxisAlignment.center : MainAxisAlignment.start,
          children: [
            Icon(icon, size: 16, color: active ? t.c.fg : t.c.fg2),
            if (!mini) ...[
              const SizedBox(width: 9),
              Expanded(
                child: Text(
                  label,
                  style: AidogType.body.copyWith(
                    fontSize: 13,
                    color: active ? t.c.fg : t.c.fg2,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              if (badge != null && badge! > 0)
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                  decoration: BoxDecoration(
                    color: t.c.accent,
                    borderRadius: BorderRadius.circular(AidogRadius.pill),
                  ),
                  child: Text(
                    badge! > 99 ? '99+' : '$badge',
                    // badge 底是 accent（两套模式下都偏深），文字取深色模式的 fg。
                    // 仍是 token 值，不是硬编码 —— token 表没有 on-accent 这一项。
                    style: numStyleSmall(AidogColors.dark.fg),
                  ),
                ),
              if (chevron != null) _Chevron(open: chevron!, color: t.c.fg3),
            ],
          ],
        ),
      ),
    );
  }
}

/// badge 数字：等宽 + 10px，色值来自 token。
TextStyle numStyleSmall(Color color) =>
    AidogType.numSm.copyWith(fontSize: 10, color: color);

class _FootButton extends StatelessWidget {
  const _FootButton({
    super.key,
    required this.mini,
    required this.icon,
    required this.label,
    required this.onTap,
  });

  final bool mini;
  final IconData icon;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: _Tappable(
        onTap: onTap,
        radius: AidogRadius.sm,
        padding: EdgeInsets.symmetric(
          vertical: 7,
          horizontal: mini ? 0 : AidogSpace.smd,
        ),
        child: Row(
          mainAxisAlignment:
              mini ? MainAxisAlignment.center : MainAxisAlignment.start,
          children: [
            Icon(icon, size: 14, color: t.c.fg2),
            if (!mini) ...[
              const SizedBox(width: AidogSpace.ssm),
              Expanded(
                child: Text(
                  label,
                  style: AidogType.label.copyWith(color: t.c.fg2, fontSize: 12),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _Chevron extends StatelessWidget {
  const _Chevron({required this.open, required this.color});

  final bool open;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return AnimatedRotation(
      duration: AidogMotion.fast,
      curve: AidogMotion.easeStandard,
      turns: open ? 0.25 : 0,
      child: Icon(Icons.chevron_right, size: 14, color: color),
    );
  }
}

/// 一块可点的表面。hover 时 surface-2 底（规则 4：hover 不发光）。
class _Tappable extends StatefulWidget {
  const _Tappable({
    required this.child,
    required this.onTap,
    required this.padding,
    this.radius = 0,
    this.background,
    this.border,
    this.shadow,
  });

  final Widget child;
  final VoidCallback onTap;
  final EdgeInsetsGeometry padding;
  final double radius;
  final Color? background;
  final Color? border;
  final List<BoxShadow>? shadow;

  @override
  State<_Tappable> createState() => _TappableState();
}

class _TappableState extends State<_Tappable> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onTap,
        behavior: HitTestBehavior.opaque,
        child: AnimatedContainer(
          duration: AidogMotion.fast,
          curve: AidogMotion.easeStandard,
          padding: widget.padding,
          decoration: BoxDecoration(
            color: widget.background ?? (_hover ? t.c.surface2 : null),
            border: widget.border == null ? null : Border.all(color: widget.border!),
            borderRadius: BorderRadius.circular(widget.radius),
            boxShadow: widget.shadow,
          ),
          child: widget.child,
        ),
      ),
    );
  }
}
