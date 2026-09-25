/// 侧栏 = 浮起玻璃卡（用户 2026-09-24 裁决，推翻票 10 A′「第 0 列不能是独立面板」）。
///
/// 对齐 React `Sidebar.tsx:250-260` 的 `glass glass-highlight`：surface 底 + 1px line
/// 边 + radius-lg 16 + shadow-sm，宽 200、padding 16/10、gap 4。logo 28px + 17px/700
/// 标题在侧栏内（`Sidebar.tsx:263-283`），壳内无独立标题栏。
///
/// 沿用现仓的信息结构（5 个 section / 可折叠 / 13 个设置子页分 5 组 / badge / 底部主题与
/// 语言切换），不沿用它的实现（自写 Dropdown、11 个内联 SVG）。
/// A′ 特有保留：56px 折叠态（React 无折叠侧栏）。
library;

import 'package:flutter/material.dart';

import '../pages/ui_bits.dart' show HoverLift;
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
    this.locales = const [],
    this.onSelectLocale,
  });

  /// 语言下拉的候选（locale code）。非空 + [onSelectLocale] 非空时，点语言按钮
  /// 在侧栏内弹出面板（React `Sidebar.tsx:156-185,502-533` 的 `Dropdown`）；
  /// 空时退回 [onPickLocale] 回调。
  final List<String> locales;
  final void Function(String locale)? onSelectLocale;

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

  /// 节折叠态（用户 2026-09-24 加回，推翻 2026-09-21「不允许折叠」禁令）：
  /// 点击节头切换；active 所在的节永远展开（同 Sidebar.tsx:301 的 `&& !activeInSection`）。
  final Map<String, bool> _sectionCollapsed = {};

  String get _topId => widget.activeId.split('/').first;

  /// 语言下拉浮层（React `Sidebar.tsx:156-185` 的 `Dropdown`：向**上**弹出的
  /// `glass-elevated` 面板 + 一层全屏遮罩接点击关闭）。
  final LayerLink _langLink = LayerLink();
  OverlayEntry? _langEntry;

  @override
  void dispose() {
    _closeLang();
    super.dispose();
  }

  void _closeLang() {
    _langEntry?.remove();
    _langEntry = null;
  }

  void _toggleLang() {
    if (_langEntry != null) {
      setState(_closeLang);
      return;
    }
    final onSelect = widget.onSelectLocale;
    // 没接候选表时退回旧回调（Rail 的 widget 测试就是这么用的）。
    if (onSelect == null || widget.locales.isEmpty) {
      widget.onPickLocale();
      return;
    }
    _langEntry = OverlayEntry(builder: (context) => _langPanel(onSelect));
    Overlay.of(context).insert(_langEntry!);
    setState(() {});
  }

  Widget _langPanel(void Function(String) onSelect) {
    final t = AidogTheme.of(context);
    return Stack(
      children: [
        // 全屏遮罩：点外面关掉（`Sidebar.tsx:162-165`）。
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () => setState(_closeLang),
          ),
        ),
        CompositedTransformFollower(
          link: _langLink,
          // `bottom: 100%; marginBottom: 6`（`Sidebar.tsx:171-173`）：向上弹。
          targetAnchor: Alignment.topLeft,
          followerAnchor: Alignment.bottomLeft,
          offset: const Offset(0, -6),
          child: Align(
            alignment: AlignmentDirectional.bottomStart,
            child: Material(
              type: MaterialType.transparency,
              child: Container(
                constraints: const BoxConstraints(minWidth: 180),
                padding: const EdgeInsets.all(AidogSpace.ssm), // padding 6
                decoration: BoxDecoration(
                  color: t.c.surface2,
                  border: Border.all(color: t.c.line),
                  borderRadius: BorderRadius.circular(AidogRadius.md),
                  boxShadow: t.shadowFloat,
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (final loc in widget.locales)
                      _Tappable(
                        key: Key('rail-locale-$loc'),
                        onTap: () {
                          setState(_closeLang);
                          onSelect(loc);
                        },
                        radius: AidogRadius.sm,
                        padding: const EdgeInsets.symmetric(
                          horizontal: AidogSpace.smd,
                          vertical: 7,
                        ),
                        background: loc == widget.localeLabel
                            ? t.c.accentWash
                            : null,
                        child: Text(
                          widget.t('lang.$loc'),
                          style: AidogType.label.copyWith(
                            fontSize: 12,
                            color: loc == widget.localeLabel
                                ? t.c.accentText
                                : t.c.fg2,
                          ),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final mini = widget.collapsed;
    return AnimatedContainer(
      duration: AidogMotion.base,
      curve: AidogMotion.easeStandard,
      width: mini ? AidogLayout.railWCollapsed : AidogLayout.railW,
      // React aside：padding 16px 10px、gap 4（Sidebar.tsx:258-262）。16 不在
      // 6 档 space 刻度里，跟 React 字面值。
      padding: const EdgeInsets.symmetric(vertical: 16, horizontal: AidogSpace.smd),
      // React `.glass`（globals.css:251-256）：surface 底 + 1px line 边 + radius-lg
      // + shadow-sm + 顶发丝。顶发丝（inset 0 1px 0）由 1px 边近似，不另画。
      decoration: BoxDecoration(
        color: t.c.surface,
        border: Border.all(color: t.c.line),
        borderRadius: BorderRadius.circular(AidogRadius.lg),
        // token 表没有 shadow-sm（mono.ts:31-35 的三档是现仓原值未进表），按
        // mono.ts 的值解析，不现编第二套。
        boxShadow: parseShadow(t.c.shadowRailCard),
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
            _brand(t, mini),
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

  /// Logo + 应用名（React `Sidebar.tsx:263-283`）：logo 28、标题 17/700/-0.3、
  /// padding 10/12/20、gap 8。折叠态只留居中 logo。
  Widget _brand(AidogTheme t, bool mini) {
    return Padding(
      // React `padding: "10px 12px 20px"`（`Sidebar.tsx:264`）。
      padding: EdgeInsets.fromLTRB(mini ? 0 : 12, 10, mini ? 0 : 12, 20),
      child: mini
          ? Center(
              child: Image.asset(
                'assets/logo.webp',
                width: 28,
                height: 28,
                filterQuality: FilterQuality.medium,
              ),
            )
          : Row(
              children: [
                // 资产由 `node scripts/gen-flutter-icons.mjs` 从 `src-tauri/icons/`
                // 同步，与 Tauri 壳同一个真值源；漂了 `--check` 会红。
                Image.asset(
                  'assets/logo.webp',
                  width: 28,
                  height: 28,
                  filterQuality: FilterQuality.medium,
                ),
                const SizedBox(width: AidogSpace.smd - 2),
                Expanded(
                  child: Text(
                    widget.t('app.title'),
                    style: AidogType.title.copyWith(
                      color: t.c.fg,
                      fontSize: 17,
                      fontWeight: FontWeight.w700,
                      letterSpacing: -0.3,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ),
    );
  }

  List<Widget> _sections(AidogTheme t, bool mini) {
    final sections = groupAdjacent(widget.items, (NavItem i) => i.section ?? '');
    final out = <Widget>[];
    for (final sec in sections) {
      // 空 section key = 平铺区，不渲染节头（与 Sidebar.tsx:289 同规则）。
      final hasHeader = sec.key.isNotEmpty && !mini;
      // active 所在的节不吃折叠（Sidebar.tsx:299-302）。
      final activeInSection = sec.items.any(
        (i) => i.id == _topId || widget.activeId.startsWith('${i.id}/'),
      );
      final collapsed = (_sectionCollapsed[sec.key] ?? false) && !activeInSection;
      if (hasHeader) {
        out.add(_sectionHead(t, sec.key, collapsed));
      }
      if (!hasHeader || !collapsed) {
        for (final item in sec.items) {
          out.addAll(_navItem(t, item, mini));
        }
      }
    }
    return out;
  }

  /// 节头：10/700/ls .5 全大写，可点折叠（用户 2026-09-24 加回），
  /// 对齐 `Sidebar.tsx:296-320`。折叠态箭头收起（React rotate(-90deg)）。
  Widget _sectionHead(AidogTheme t, String key, bool collapsed) {
    return _Tappable(
      key: Key('rail-section-$key'),
      onTap: () => setState(
        () => _sectionCollapsed[key] = !(_sectionCollapsed[key] ?? false),
      ),
      radius: AidogRadius.sm,
      // React 节头 padding 8px 10px 4px（Sidebar.tsx:302）；8/4 不在 6 档 space
      // 刻度里，这里跟 React 字面值，不往档上凑。
      padding: const EdgeInsets.fromLTRB(AidogSpace.smd, 8, AidogSpace.smd, AidogSpace.sxs),
      child: Row(
        children: [
          Expanded(
            child: Opacity(
              opacity: 0.7,
              child: Text(
                widget.t(key).toUpperCase(),
                style: AidogType.micro.copyWith(
                  fontSize: 10,
                  fontWeight: FontWeight.w700,
                  letterSpacing: 0.5,
                  color: t.c.fg3,
                ),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ),
          Opacity(
            opacity: 0.5,
            child: _Chevron(open: !collapsed, color: t.c.fg3),
          ),
        ],
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
        // React 子容器 `paddingLeft: 12`（`Sidebar.tsx:406`）。
        padding: const EdgeInsetsDirectional.only(
          start: 12,
          top: 2,
          bottom: AidogSpace.sxs,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final g in groups) ...[
              // 组标题：10 w600 ls0.3 secondary + opacity .6、**不大写**、
              // padding 6px 10px 2px（`Sidebar.tsx:416-425`）。
              Padding(
                padding: const EdgeInsets.fromLTRB(10, 6, 10, 2),
                child: Opacity(
                  opacity: 0.6,
                  child: Text(
                    widget.t(g.key),
                    style: AidogType.micro.copyWith(
                      fontSize: 10,
                      fontWeight: FontWeight.w600,
                      letterSpacing: 0.3,
                      color: t.c.fg2,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
              for (final c in g.items)
                _Tappable(
                  onTap: () => widget.onNavigate(c.id),
                  // React 子项 padding 7px 10px 7px 26px（Sidebar.tsx:431）。
                  padding: const EdgeInsetsDirectional.only(
                    start: 26,
                    top: 7,
                    bottom: 7,
                    end: AidogSpace.smd,
                  ),
                  radius: AidogRadius.sm,
                  background: widget.activeId == c.id ? t.c.liveFill : null,
                  // 子项左缘 2px：活跃 accent-edge、非活跃透明（位置照占，
                  // `Sidebar.tsx:441`）。
                  startBar: widget.activeId == c.id
                      ? t.c.accentEdge
                      : Colors.transparent,
                  lift: widget.activeId != c.id, // Sidebar.tsx:428
                  child: Text(
                    widget.t(c.labelKey),
                    // React 子项 12.5px、活跃 w600（Sidebar.tsx:432-433）。
                    style: AidogType.caption.copyWith(
                      fontWeight: widget.activeId == c.id
                          ? FontWeight.w600
                          : FontWeight.w400,
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
      // React 底部区 paddingTop 12（Sidebar.tsx:465-471）。
      padding: const EdgeInsets.only(top: 12),
      decoration: BoxDecoration(border: Border(top: BorderSide(color: t.c.line))),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        spacing: 4, // React 底部区 gap 4（Sidebar.tsx:465-471）
        children: [
          _FootButton(
            key: const Key('rail-theme-toggle'),
            mini: mini,
            // React 主题图标 16（Sidebar icons.sun/moon，Sidebar.tsx:465-498）。
            icon: widget.isDark ? Icons.dark_mode_outlined : Icons.light_mode_outlined,
            iconSize: 16,
            label: widget.t(widget.isDark ? 'theme.dark' : 'theme.light'),
            onTap: widget.onToggleTheme,
            // 右端 12×12 色点：白 / #0a0a0b 填充 + 1px border 光环
            //（`Sidebar.tsx:491-498`）。
            trailing: mini
                ? null
                : Container(
                    width: 12,
                    height: 12,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: widget.isDark
                          ? AidogColors.dark.bg
                          : AidogColors.light.surface,
                      border: Border.all(color: t.c.line),
                    ),
                  ),
          ),
          CompositedTransformTarget(
            link: _langLink,
            child: _FootButton(
              key: const Key('rail-locale'),
              mini: mini,
              // React IconGlobe size 14。
              icon: Icons.language_outlined,
              label: widget.localeLabel,
              onTap: _toggleLang,
              // 右端 chevron，opacity .4（`Sidebar.tsx:517`）。
              trailing: mini
                  ? null
                  : Opacity(
                      opacity: 0.4,
                      child: Icon(
                        Icons.keyboard_arrow_down,
                        size: 14,
                        color: t.c.fg2,
                      ),
                    ),
            ),
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
        // 活跃项左缘 2px 竖条：`inset 2px 0 0 var(--accent-edge)`（`Sidebar.tsx:345`）。
        startBar: active ? t.c.accentEdge : null,
        // React 只给非活跃项挂 hover-lift（`Sidebar.tsx:331`）。
        lift: !active,
        // 深色下发光，浅色下 token 把 halo 置 none、由 fill + edge 接替。
        shadow: active ? t.liveHalo : null,
        // React 行 padding 10px 12px（Sidebar.tsx:335）。
        padding: EdgeInsets.symmetric(
          vertical: mini ? 7 : 10,
          horizontal: mini ? 0 : 12,
        ),
        child: Row(
          mainAxisAlignment:
              mini ? MainAxisAlignment.center : MainAxisAlignment.start,
          children: [
            // React 图标 18（Sidebar.tsx:13），非活跃 60% 透明（Sidebar.tsx:373-378）。
            // mini 时包 Flexible：折叠动画中 _Tappable 的水平 padding 从 12 渐变到 0，
            // 中段行宽会短暂小于 18，不兜就抛 overflow 断言（终态 36 宽不受影响）。
            // 非活跃靠 opacity 0.6 压，不换色（`Sidebar.tsx:371`）。
            mini
                ? Flexible(
                    child: Opacity(
                      opacity: active ? 1 : 0.6,
                      child: Icon(icon, size: 18, color: active ? t.c.fg : t.c.fg2),
                    ),
                  )
                : Opacity(
                    opacity: active ? 1 : 0.6,
                    child: Icon(icon, size: 18, color: active ? t.c.fg : t.c.fg2),
                  ),
            if (!mini) ...[
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  label,
                  style: AidogType.body.copyWith(
                    fontSize: 13,
                    // React 活跃 w600（Sidebar.tsx:336）。
                    fontWeight: active ? FontWeight.w600 : FontWeight.w400,
                    color: active ? t.c.fg : t.c.fg2,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              if (badge != null && badge! > 0)
                Container(
                  // React 盒型：minWidth 16 / height 16 / padding 0 5px
                  //（`Sidebar.tsx:387-399`）。
                  constraints: const BoxConstraints(minWidth: 16, minHeight: 16),
                  alignment: Alignment.center,
                  padding: const EdgeInsets.symmetric(horizontal: 5),
                  decoration: BoxDecoration(
                    // React badge 亮底深字（Sidebar.tsx:385-402）：底 --accent
                    // = accent-text（深色 #E6E8EC 亮），字 --accent-foreground
                    // （mono.ts:72，深色 bg / 浅色 surface）。
                    color: t.c.accentText,
                    borderRadius: BorderRadius.circular(AidogRadius.pill),
                  ),
                  child: Text(
                    badge! > 99 ? '99+' : '$badge',
                    style: numStyleSmall(
                      t.mode == AidogMode.dark ? t.c.bg : t.c.surface,
                    ),
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

/// badge 数字：React 是 **sans** 10 w700（`Sidebar.tsx:387-389`），不是等宽族。
TextStyle numStyleSmall(Color color) => AidogType.micro.copyWith(
  fontSize: 10,
  fontWeight: FontWeight.w700,
  letterSpacing: 0,
  color: color,
);

class _FootButton extends StatelessWidget {
  const _FootButton({
    super.key,
    required this.mini,
    required this.icon,
    required this.label,
    required this.onTap,
    this.iconSize = 14,
    this.trailing,
  });

  final bool mini;
  final IconData icon;
  final String label;
  final VoidCallback onTap;

  /// 行尾挂件：主题键的 12×12 色点、语言键的 chevron
  /// （`Sidebar.tsx:491-498,517`）。mini 态两者都不画，调用方传 null。
  final Widget? trailing;

  /// React 底部按钮图标 16/14（主题 16、语言 14，Sidebar.tsx:465-498）。
  final double iconSize;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    return _Tappable(
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
          Icon(icon, size: iconSize, color: t.c.fg2),
          if (!mini) ...[
            const SizedBox(width: AidogSpace.ssm),
            Expanded(
              child: Text(
                label,
                style: AidogType.label.copyWith(color: t.c.fg2, fontSize: 12),
                overflow: TextOverflow.ellipsis,
              ),
            ),
            if (trailing != null) ...[
              const SizedBox(width: AidogSpace.ssm),
              trailing!,
            ],
          ],
        ],
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
    super.key,
    required this.child,
    required this.onTap,
    required this.padding,
    this.radius = 0,
    this.background,
    this.border,
    this.shadow,
    this.startBar,
    this.lift = true,
  });

  final Widget child;
  final VoidCallback onTap;
  final EdgeInsetsGeometry padding;
  final double radius;
  final Color? background;
  final Color? border;
  final List<BoxShadow>? shadow;

  /// 左缘 2px 竖条。活跃 nav 项是 `inset 2px 0 0 var(--accent-edge)`
  /// （`Sidebar.tsx:344-347`），子项是 `borderLeft: 2px solid`（`:441`，
  /// 非活跃时透明，位置照占）。
  final Color? startBar;

  /// hover 抬升。React 只给**非活跃**项挂 `hover-lift`
  /// （`Sidebar.tsx:331,428`：`isActive ? "ripple" : "ripple hover-lift"`）。
  final bool lift;

  @override
  State<_Tappable> createState() => _TappableState();
}

class _TappableState extends State<_Tappable> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final radius = BorderRadius.circular(widget.radius);
    final ring = widget.border;
    final bar = widget.startBar;
    final surface = MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: AnimatedContainer(
        duration: AidogMotion.fast,
        curve: AidogMotion.easeStandard,
        // 竖条画在容器**内部**（React 用的是 inset shadow，同样被圆角裁），
        // 所以这里开裁剪、把它当行首的一个 2px 格子排。
        clipBehavior: bar == null ? Clip.none : Clip.antiAlias,
        decoration: BoxDecoration(
          color: widget.background ?? (_hover ? t.c.surface2 : null),
          border: ring == null ? null : Border.all(color: ring),
          borderRadius: radius,
          boxShadow: widget.shadow,
        ),
        // 涟漪：React 每颗侧栏按钮都挂 `ripple`（`Sidebar.tsx:331,428,477`）。
        child: Material(
          type: MaterialType.transparency,
          child: InkWell(
            onTap: widget.onTap,
            borderRadius: radius,
            child: bar == null
                ? Padding(padding: widget.padding, child: widget.child)
                : Stack(
                    children: [
                      Padding(padding: widget.padding, child: widget.child),
                      // 左缘 2px：`inset 2px 0 0 var(--accent-edge)`
                      //（`Sidebar.tsx:345`）／子项的 `borderLeft`（`:441`）。
                      // 与 inset shadow 同样是**盖**在内容上，不挤走文字。
                      PositionedDirectional(
                        start: 0,
                        top: 0,
                        bottom: 0,
                        width: 2,
                        child: IgnorePointer(child: ColoredBox(color: bar)),
                      ),
                    ],
                  ),
          ),
        ),
      ),
    );
    return widget.lift ? HoverLift(child: surface) : surface;
  }
}
