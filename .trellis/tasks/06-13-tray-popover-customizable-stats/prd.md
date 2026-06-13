# PRD: tray popover 浮窗可自定义统计展示

## 背景

tray 左击弹 popover 浮窗 (research/01-04):
- 已有基础: `src/popover.tsx` + `popover.html` + `vite.config.ts:33` 多入口; 数据来自 `popover_data` command(lib.rs:301); 窗口左击创建/toggle(lib.rs:2295, 已修 scale 定位 + Down-only, commit `6e0f5f0`)。
- 现展示 = 代理状态行 + tray 配置平台列(余额/coding) + **硬编码**今日 4 格(tokens/cost/cache%/reqs)。
- 今日金额/缓存率/token 总量 = `today_stats`(db.rs:524) **已有可复用**; **各平台当日已用 = 缺**(get_platform_usage_stats 是累计全时段无 GROUP BY, 需新增)。
- 设置: AppSettings tray tab → `TrayConfigTab.tsx`(742 行, 拖拽/预览/工厂); `TrayConfig{items}`(models.rs:636) 存 settings scope="tray"/key="config"(db.rs:476)。

## 目标

popover 浮窗内容**完全由用户配置驱动**, 默认展示: ① 今日已用金额 ② 缓存率 ③ Token 总量 ④ 各平台当日使用(只含已用)。用户可在设置里**显隐 + 排序 + 增删项**(粒度 C)。

## 决策 (已确认)

1. **自定义粒度 C**: 显隐 + 拖拽排序 + 增删项。
2. **内容全配置驱动**: 现有代理状态行 / 平台列也纳入为可配置 item type; 默认配置含上述 4 项(+ 代理状态等按默认)。
3. "增删项" = 从**预定义指标集**自由组合增删/排序/显隐(非用户输入任意数据源)。

## 数据结构

```
PopoverItemType (预定义指标集):
  today_cost          今日已用金额
  today_cache_rate    今日缓存率
  today_tokens        今日 token 总量
  platform_today      各平台当日使用 (只含已用, 列表)
  proxy_status        代理状态行 (现有)
  platform_balance    平台余额/coding 列 (现有 tray 列)
  (后续可扩展)

PopoverItem { id: string, type: PopoverItemType, visible: bool, order: int }
PopoverConfig { items: PopoverItem[] }
  持久化: settings scope="popover" key="config" (仿 TrayConfig, 零 schema 改)
  默认: [today_cost, today_cache_rate, today_tokens, platform_today] 可见 + 现有项
```

## 后端

- 复用 `today_stats`(db.rs:524): 金额(SUM est_cost) / 缓存率(cache/input) / token(SUM input+output)。
- **新增** `today_platform_stats`(db.rs): `GROUP BY platform_id WHERE created_at>=今日起 AND deleted_at=0`, 只返回有用量的(已用)平台; **platform_id=0 经 `group.auto_from_platform` 回溯平台名**(db.rs:1310 模式), 否则归"未知平台"。
- `popover_data` command(lib.rs:301) 改为读 PopoverConfig + 按所含 item type 返回对应数据 + 配置本身。
- PopoverConfig get/set command(仿 tray config CRUD)。
- models.rs: PopoverItem/PopoverConfig 结构 + TS 类型同步。

## 前端

- `popover.tsx`: 按 `config.items` 的 order + visible 顺序渲染各 type(替换现有硬编码布局, 改为配置驱动 map)。
- 设置: 新增 `PopoverConfigTab.tsx`(仿 `TrayConfigTab.tsx` 拖拽/显隐/增删/预览范式), 挂到 AppSettings(tray tab 旁或新 popover tab)。
- i18n: item type 标签 + 设置文案 7 语言齐。

## 非目标

- 不改 tray 双行图标显示(只改左击 popover)。
- 不引入用户输入任意数据源的"自定义项"(预定义指标集内增删)。
- 不改 `6e0f5f0` 的 scale 定位 / Down-toggle / ph=420 逻辑(衔接不覆盖)。

## 风险 (research/04)

- 各平台当日: proxy_log 只有 platform_id, 需经 auto_from_platform 回溯平台名(platform_id=0 自动分组)。
- 语义标注: ④"当日" vs 平台页"累计"不同, popover 内需明示"今日"。
- 与 tray wip 衔接: 改 popover_data 不冲突, 但别覆盖 lib.rs:2313/2317 scale 行 + ph。
- TrayConfigTab 范式复用: 抽公共拖拽组件还是复制? 遵 code-reuse 优先抽共享。

## 验收标准

- popover 默认展示今日金额/缓存率/token/各平台当日(只含已用), 数据与主窗口同源一致。
- 设置可显隐 + 拖拽排序 + 增删 popover 项, 持久化(scope="popover"), popover 实时反映。
- 各平台当日: platform_id=0 正确回溯平台名, 不归"未知"; 无用量平台不显示。
- cargo build + cargo test + yarn tsc 0 error 无新增 warning; 7 语言 i18n 齐。
- 不破坏 `6e0f5f0` popover 定位/toggle。
- Rust↔TS 契约一致(PopoverConfig/PopoverItem)。

## 编排

单一交付(popover 配置驱动 + 设置 UI), 单 worktree。改动跨后端(today_platform_stats + PopoverConfig command + popover_data)+ 前端(popover.tsx + PopoverConfigTab)+ i18n, 强耦合(配置契约)。不拆 child。**依赖**: 与重试 task 同碰 lib.rs(popover_data vs platform command, 区段不同但同文件)→ **等重试 task merge 后再 start**, worktree 基线同步。串行排在重试之后(与 group-scoped 可任意先后, 都在重试后)。
