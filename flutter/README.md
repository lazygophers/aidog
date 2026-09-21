# aidog Flutter 外壳

桌面主窗口的 Flutter 实现（票 I01 起）。后端一行不换 —— 通过 `aidog-kernel` 的
HTTP RPC + SSE 说话。目标平台只有 **macOS 与 Windows**（对齐 `release.yml:36-44` 的构建矩阵）。

## 其余 13 张票只需要知道这些

```dart
import 'package:aidog_flutter/transport.dart';

await kernel.start();                       // 拉起 aidog-kernel，等到可用
final info = await kernel.invoke<Map<String, dynamic>>('about_info');
kernel.on<int?>('proxy-log-updated').listen(refresh);   // 单连接扇出
kernel.states.listen(...);                  // connecting / connected / reconnecting / stopped
```

| 要什么 | 用什么 |
|---|---|
| 调命令 | `kernel.invoke<T>(cmd, args)` |
| 订事件 | `kernel.on<T>(eventName)` |
| 连接状态（渲染「后端连接中」） | `kernel.state` / `kernel.states` |
| 命令失败 | `catch (e)`，`e` 是 `RpcException`，`e.body` = 错误值本身 |
| 连不上 | `RpcTransportException` |

**禁止**自己开 HTTP 连接。`package:http` / `dart:io` 的 `HttpClient` 打 RPC 比持久 socket
慢一个数量级。

## 图表层（票 I04）

```dart
import 'package:aidog_flutter/charts.dart';
```

页面票（I06 首页 / 统计，I07 平台 / 分组 / 日志）从这里取图表，**卡片壳和图例仍归
`SeriesTile`**（票 I02 的四种格子之一，本层不造第五种）。

| 要什么 | 用什么 |
|---|---|
| 折线 / 双轴 / mini 曲线 | `AidogLineChart(series: [...], area: true, mini: false)` |
| 堆叠面积 | `AidogStackedAreaChart(series: [...])` |
| 环形占比 | `AidogDonutChart(data: [...], topN: 4)` |
| 散点直方图 | `AidogScatterChart(histogram: ...)` |
| 时刻热力（24×7）/ 迷你热力条 / 维度热力 | `HourHeatmap` / `HourHeatBar` / `DimensionHeatmap` |
| 仪表盘 + 趋势 sparkline | `GaugeChart(value:, max:, trend: [...])` |
| 取色 | `ChartPalette.of(context).series(i)` / `.heat(t)` |
| 图例喂给 `SeriesTile` | `legendOf(series)` |
| 空态 | 图表自己渲染 `ChartEmpty(emptyText)`，不画零值假图 |

### 一条序列 = 一个 `ChartSeries`，身份不跟位置走

```dart
ChartSeries(
  key: 's0',                 // 稳定身份（Stats 的安全键），不是位置
  label: '深度求索',          // 展示名
  color: palette.series(0),
  points: rows,              // 按 x 升序
  format: formatCostUsd,     // 本序列自己的格式化
  rightAxis: false, dashed: false,
);
```

**为什么这么设计**：fl_chart 的 tooltip 回调只给位置索引（`barIndex`），而 Stats 的序列
顺序按总量动态排（`src/pages/Stats.tsx:114`）。位置 + 动态排序 = 格式化静默走偏，
显示一个错的数还不报错 —— commit `a7e665c8` 刚修掉的就是这个形状。这里把
**身份 / 展示名 / 颜色 / 格式化挂在同一个对象上**，没有第二份按位置对齐的平行数组可漂移。

取行一律走 `tooltipRowAtSpot(series, barIndex, spotIndex)`（数值从序列自己的点集里读，
不信任绘图坐标）或 `tooltipRowFor(series, key, value)`。越界抛 `RangeError`，key 不存在抛
`ArgumentError` —— 不静默回落。`test/charts/tooltip_test.dart` 里「按总量重排后」那一组
是复发闸：把序列顺序整个反过来，每条的名字 / 颜色 / 格式化仍必须是它自己的。

### 迁了 8 个，没迁 3 个

- **fl_chart**（MIT，与本项目 AGPL-3.0-or-later 相容；**Syncfusion 专有许可不可用**）：
  折线 / 堆叠面积 / 环形 / 散点。
- **不需要图表库**：三张热力图 —— React 那边本来就是 CSS grid 的 div 格子
  （文件头写着「零 canvas 零 Recharts」），这里是格子布局不是图表库。
- **`CustomPainter`**：仪表盘圆环（`canvas.drawArc`）+ 趋势 sparkline。
- **没迁**：`BarChart` / `PieChart` / `StackedBarChart`（共 276 行）在 React 侧**零页面
  调用者**，只被 `index.ts` 和自己的测试引用。没有消费者就不迁。

### 两处实现差异，写下来

1. **fl_chart 没有第二 Y 轴**（上游 issue #429 仍 open）。右轴序列的值被线性映射进左轴
   坐标系再画（`mapToLeft`），右轴刻度反解回原值标注，tooltip 走 `tooltipRowAtSpot`
   取原值。唯一的双轴消费者 `src/pages/Home.tsx:362` 是 mini 态、轴本来就隐藏。
2. **fl_chart 没有 `stackId`**。堆叠是 `AidogStackedAreaChart` 自己累加出来的，且
   **绘制顺序整体反转**（栈顶先画，否则上层填充盖住下层）。反的是 `ChartSeries` 列表
   本身，tooltip 查的是同一个列表。

## 页面层（票 I06 起）

```dart
import 'package:aidog_flutter/pages.dart';
```

`main.dart` 的 `pageBuilder` 按 `activeId` 分发；已落地的页面走真实现，其余仍是占位。

**三条约定，后续页面票（I07-I09）沿用，别各发明一套**：

1. **没有第二套 api 封装**。页面直接 `kernel.invoke('cmd', {...})` 传内联 map，
   字段名从 `src/services/api/types/generated/<Type>.ts` 抄（理由见下面「不生成 203 个
   Dart 绑定」）。模型类放 `lib/src/pages/models.dart`（页面私有）或 `lib/stats/models.dart`
   （统计 wire 类型）。
2. **每个页面收一个 `InvokeFn`**（缺省 `kernelInvoke`），widget 测试塞假实现 ——
   全局 `kernel` 是 `final`，不留这个口子就没法在不起内核的情况下测页面。
   事件同理：页面收一个 `Stream<void>? logUpdates`，缺省是
   `debounceStream(kernelProxyLogUpdated())`（500 ms 防抖，对齐 React 的 `onProxyLogUpdated`）。
3. **状态就是 `StatefulWidget` + `setState`**。React 那边是 `useState` + `useEffect`，
   再套一层 controller 就是只有一个实现的抽象。

### 对齐是怎么证的：命令覆盖零差集

`test/pages/command_coverage_test.dart` 把页面沿**所有路径**走一遍（统计页四个 tab +
两个密度视图），把实际发出的命令名收成**排序后的串**，与从 React 版逐行读出来的清单
逐字比对。差一条 = 少一块功能，多一条 = 多一次 React 没有的 IPC，两边都红。

> 比的是**串**不是 `Set`：Dart 的 Set / List / Map 按身份比较，`expect(setA, setB)`
> 永远不等（本项目踩过，2826 个 key 全报不等）。

首页 6 条、统计页 6 条、共用 2 条、并集 10 条。清单与每条的 React 侧出处写在那个文件抬头。

### widget 测试的两个坑

- **别用 `pumpAndSettle`**：骨架的 `LiveDot` 是 3 秒无限循环呼吸动画，永远等不到静止。
  用 `test/pages/harness.dart` 的 `settle(tester)`（推几帧）。
- **画布默认只有 800×600**，统计页一屏放不下，`tap()` 会判成「点不到」。
  交互测试先 `await useBigSurface(tester)`。

### 与 React 版的已知差异（写下来，不是漏的）

| 处 | React | 这里 | 为什么 |
|---|---|---|---|
| 热力图星期标签 | `Intl.DateTimeFormat(lang,{weekday:'short'})`，「周日 / Sun」 | `MaterialLocalizations.narrowWeekdays`，单字 | 8 语言词条由 `flutter_localizations` 自带、零运行时初始化；`intl` 的 `DateFormat.E` 得先 `await initializeDateFormatting()`，漏调就在非英文 locale 下抛 `LocaleDataException` |
| 分页文案占位符 | 传 `{page, total}`，而词条写的是 `{{current}}` —— **React 现在渲染出的是没替换的 `{{current}}`** | 传 `{current, total}`，正常替换 | 那是 React 侧的真 bug，不照抄 |
| 入场动效 | `useReveal` 错峰 + `useCounter` 数字滚动 | 无 | A′ 的格子有自己的过渡；票 I06 的口径是「对齐」指功能不指长相 |

## 页面批次 B（票 I07）：平台 / 分组 / 请求日志 / 日志

```dart
import 'package:aidog_flutter/pages.dart';   // PlatformsPage / GroupsSection / LogsPage / RequestLogPage
```

四页都交付了**逻辑层 + widget 树**（不是只有逻辑层）。分层沿用 I06 的三条约定：
`XController` 持状态与命令、`StatefulWidget` 只负责画、页面收一个 `InvokeFn` 供测试注入。

| 文件 | 管什么 |
|---|---|
| `lib/src/pages/platforms_logic.dart` | 平台列表 / 乐观写的 epoch 守卫 / 余额有界并发 / 取模型的多协议回退链 |
| `lib/src/pages/groups_logic.dart` | 分组分页加载 / 校验 / 四个批量操作 / 破坏性确认 / 一键测试 |
| `lib/src/pages/logs_logic.dart` | 两页的筛选派生、分页、详情、复制成 markdown |
| `lib/src/pages/{platforms,groups,logs}.dart` + `ui_bits.dart` | widget 树 |

### SSE 只是「有新数据」的提示，不是数据源

`proxy-log-updated` 对**连接之前**发生的事件什么都不补，断线期间的也全丢。所以两条硬规矩：

1. **每次 mount 必须自己整查一遍**（`init()` 里就查，不等流）；
2. 流上来的每一下只触发一次**静默**重查（不闪 loading）。

React 侧票 11 量到过「转发期一秒四次整页重查」，根因是订阅没防抖。这里两道闸：
I06 的 `debounceStream`（500 ms 尾沿）+ 控制器内的 `_inFlight`（在途一轮没回来就不发第二轮）。
`test/pages/logs_logic_test.dart` 里「四次事件叠在一起时只该落地一次查询」那条是复发闸。

### 命令覆盖：45 / 51，缺的 6 条逐条写明

清单由 `scripts/i07-react-pages-b-commands.mjs` 从 `src/` **解析**出来（不是手数）：
命名空间对象只算本文件真访问过的成员，所以 import 一个 `platformApi` 不会把它 20 个方法全算进来。
产物 `flutter/test/pages/react_pages_b_commands.txt`，比对在 `test/pages/command_coverage_b_test.dart`
（比**排序后的 List**，不是 Set —— Dart 的 Set 按标识比较）。

解析器本身与票 I08 共用 `scripts/lib/react-commands.mjs`；抽出后 I08 那份 96 条清单
`--check` 仍逐字节相等。

没覆盖的 6 条，全部写在测试里的 `knownGaps` 表并**计入总数**（不是从清单里删掉）：

| 命令 | 为什么没做 |
|---|---|
| `get_client_types_json` | 端点的「客户端形态」下拉。registry 已删 `client_type` 字段，形态按 protocol 派生，本层不读这份表 |
| `get_protocol_logo_path` / `get_protocol_logo_data_url` / `sync_protocol_logo` | 协议图标三件套。图标由 shell 主题层出，未走这条链 |
| `settings_get` | 被 `domains/groups/proxy-env.ts` 用来拼分组的环境变量预览文本；本层没做那个预览面板 |
| `sync_group_settings` | 「一键同步到 ~/.claude/settings.{group}.json」按钮，不属分组 CRUD，未随本票交付 |

### 与 React 版的已知差异（写下来，不是漏的）

| 处 | React | 这里 | 为什么 |
|---|---|---|---|
| 余额查询的入队顺序 | `IntersectionObserver` 按卡片进视口的顺序（可视优先） | 列表顺序 | Flutter 没有等价的廉价原语。并发上限、去重、pending 三态都一样，差的只是**先查哪个** |
| 该不该查余额 | 还要过 `platformHasQuotaScript`（registry 的 `quota_scripts` 索引） | 只判「有 key 且有 base_url」 | 那个索引是 `get_defaults_json` 的一个子树，本层没解析。后果是**多查**不是少查：没脚本的平台白发一次命令，后端返 `success:false`，UI 表现一致 |
| 平台卡 | favicon、拖拽手柄、展开端点明细 | 动作做全（启停 / 测试 / 刷余额 / 查日志 / 删除），展开明细未做 | 本票口径是功能对齐优先于观感；拖拽排序的命令（`platform_reorder`）已接，只是没有手柄 UI |
| 跨组件通知 | `window` 上三个自定义事件（`aidog-groups-changed` 等） | 父子回调 | 分组区在 Flutter 这边是平台页的**子 widget**，不是兄弟页，不需要事件总线 |
| 详情 / 确认弹窗 | Radix Sheet / AlertDialog（Portal 到 body） | 页面内的一张格子 | 项目 CLAUDE.md 那条「弹窗必须 createPortal」是 CSS 的坑（祖先 `transform` 让 `fixed` 退化），只对 Web 侧成立。做成页面 state 的一部分，widget 测试 `find.byType(ConfirmCard)` 就能断言 |
| `formatDateTime` | `toLocaleString()`，跟浏览器 locale 走 | 固定 `YYYY/M/D HH:MM:SS` | 跟 locale 走要先 `initializeDateFormatting()`，漏调会在非英文 locale 抛 `LocaleDataException` —— 与 I06 不用 `DateFormat.E` 同一个理由 |

### 平台新增 / 编辑表单（I18 补齐）

```dart
import 'package:aidog_flutter/src/pages/platform_form.dart';      // PlatformEditForm / WindowsEditor
import 'package:aidog_flutter/src/pages/platform_form_logic.dart'; // PlatformFormController
```

11 个分区全在（基础信息 / Mock 特例 / 配额脚本 / Devin / 透传 / Protocol Endpoints /
认证 / 多 key 预览 / 模型矩阵 + 时段档 / 手动预算 / 熔断 / 高峰 / 分组归属 / 过期）。
表单态在 `PlatformFormController`，列表侧依赖（平台名集合、分组清单、落库动作）
由 `PlatformsController` 注入，与 React 的 `PlatformFormListDeps` 一一对应。

`platform.extra` 的七对 parse/serialize 在 `platform_extra.dart`，registry 派生层
（默认端点 / 默认模型 / 模型候选 / peak / 配额变体 / 套餐档位 / 客户端模拟候选）
在 `platform_defaults.dart`，时段判定内核在 `time_window.dart`。

| 处 | React | 这里 | 为什么 |
|---|---|---|---|
| 过期时间 / 生效期的日期输入 | `<input type="datetime-local">`，浏览器原生日历 | 文本框，格式就是 datetime-local 那串 `YYYY-MM-DDTHH:MM` | Flutter 没有等价的单控件；`showDatePicker`+`showTimePicker` 要两次弹窗，反而比原来多一步。用户看到与输入的串一字不差 |
| 协议选择器 | Tab / Shift-Tab 在整表里循环切下一个平台、logo 图标 | 点开 → 搜 → 选 | Tab 循环依赖 Web 的焦点模型；logo 三级回退属缺口清单 C1-C3，不在本票范围 |
| 「受影响模型」输入 | `<datalist>` 自动补全 | 纯输入，回车 / 失焦成 chip | 候选列表已经在模型矩阵的下拉里；这里再挂一份是第二个真值源 |
| 「复制平台」入口 | 平台卡上的按钮 | `handleDuplicate` 已实现，卡片按钮未接 | 卡片按钮属缺口清单 #4，不在本票范围 |
| 智能粘贴识别 | 表单右上角「智能识别」按钮 | 无 | 属缺口清单 #6，不在本票范围 |

### 拼音搜索（I17 补齐）

平台 / 分组 / 筛选下拉的中文模糊搜索走 `lib/src/utils/pinyin.dart::pinyinMatch`，
匹配语义与 React 的 `pinyin-pro` 版一致（直接子串 / 全拼子串 / query 中文转拼音 /
首字母串）。词典是**自建**的 3500 常用字表（`scripts/pinyin_3500.txt`，《现代汉语
常用字表》），读音由 pinyin-pro 生成、入库为 `lib/src/utils/pinyin_data.dart`
（约 29 KB，`node scripts/gen-flutter-pinyin-data.mjs --check` 盯着不漂移）。
不引 pub.dev 拼音包：唯一候选多年未更新且许可证未核实，不往 AGPL 仓库里引。
表外生僻字按原字符保留（与 pinyin-pro 查不到时的行为一致），只可能搜不到、不会误报。

### 路由配置这一页，校验和确认是照搬的

Groups 是全库第二大页，改错一个字段不是显示问题，是请求发去错的地方。所以：

- **能不能点**：「创建」按 `!cName`、「保存」按 `!editName`（**不 trim** —— 一个空格在 React 里是可以保存的，照搬）。
- **分组密钥**实时剔掉 `[^\w-]`：它创建后锁定不可改，且同时是 Bearer token 与路由匹配键，放进去一个空格就是一个永远匹配不上的组。
- **破坏性操作一律先确认**：删组、删平台、批量删除、清理失效，确认之前一个命令都不发（widget 测试逐条盯着）。
- **跨组归属实时拉后端**：「移除平台」弹窗里「这个平台还在几个组里」必须用 `group_detail_list` 现拉。用前端已分页的 `details` 会 overcount，单组平台就会被「移出本组」变成未分组而不是被删掉 —— React 07-08 回归的根因。
- **`env_vars` 必须透传**：后端 `UpdateGroup.env_vars` 是 `#[serde(default)]` 的 `Vec`（不是 `Option`），改模型映射时不带上它就会把用户已配的环境变量清光。

## 原生能力（票 I12）

```dart
import 'package:aidog_flutter/platform.dart';
```

需求文档是 React 版的 `src/services/platform.ts:7-15` 那张表，本层按同一份语义落地。

| 你要做的事 | 调这个 | React 版对应物 |
|---|---|---|
| 复制文本 | `writeText(s)` | `platform.ts::writeText` |
| 读剪贴板 | `readText()` → 空剪贴板给 `''` | `platform.ts::readText` |
| 打开外部链接 | `openUrl(url)`，打不开会抛 | `platform.ts::openUrl` |
| 在访达 / 资源管理器里定位文件 | `revealItemInDir(path)` | `platform.ts::revealItemInDir` |
| 按钮该写「在访达中显示」还是「复制路径」 | `canRevealItemInDir()` | `platform.ts::canRevealItemInDir` |
| 应用版本号 | `await getAppVersion()` | `platform.ts::getAppVersion` |
| 选文件 / 目录 / 保存位置 | `await pickPath(PickPathOptions(...))` | `pathPicker.ts::pickPath` |
| 重启应用 | `await relaunch()`（正常不返回） | `updater.ts:83` 的 `relaunch()` |
| 弹一条系统通知 | `await notify(title, body)`，失败不抛 | `platform.ts::notify` |

两个降级，UI 要知道：

- **定位文件**：只有 macOS / Windows 有（`open -R` / `explorer /select,`）。其余 OS 上
  `revealItemInDir` **退化成把路径复制到剪贴板** —— 与 React 版的浏览器降级同一语义。
  所以按钮文案先看 `canRevealItemInDir()`，别让用户以为访达会弹出来。
- **对话框标题**：Tauri 的 `title` 设的是对话框窗口标题，`file_selector` 只有
  `confirmButtonText`。`PickPathOptions.title` 映到后者。macOS 面板本来就不显示窗口标题。

**`main()` 里必须调一次 `bindKernelPopups()`**（在 `kernel.start()` 之后）：

```dart
await kernel.start();
bindKernelPopups();     // 不调 = 后端所有通知的「系统弹窗」通道全哑
```

理由：通知的分发在 Rust 侧（`aidog_notification::dispatch`）。Tauri 外壳那边
`TauriCtx::show_popup` 直接调插件弹；无界面内核没有桌面会话，改成广播 `notif-popup`
事件（`HeadlessCtx::show_popup`），由外壳代弹。不订阅就等于把这条通道剪断。

**`fs` / `shell` / `path_provider` 不在本层**：前端对这两个 Tauri 插件零调用
（`plugin-shell` 已迁到后端命令 `mitm_install_ca`，见 `src/services/api/mitm.ts:81`；
`plugin-fs` 全库只有 `package.json` 里一行，没有 import 点）。要读写文件的都在 Rust 侧走 RPC。

### 定时 / 重复通知的三平台缺口，对现状零影响

`flutter_local_notifications` 的已知空洞是真的：macOS 不实现旧的 `schedule` /
`showDailyAtTime` / `showWeeklyAtDayAndTime`（`zonedSchedule` 有），Linux 完全没有调度 API，
Windows 的 `periodicallyShow*` 抛 `UnsupportedError`。

**本项目一条都没用到。** 全库的系统通知只有「立刻弹一条」这一种：React 版
`platform.ts::notify` 只被 `App.tsx:156` 的 `proxy-start-failed` 调用；Rust 侧
`aidog_notification::show_popup` 也只有立即弹。定时那一层在 Rust 的周期任务里，到点了才调
「立刻弹」—— 调度权从来不在通知插件手上。所以零功能损失。

`test/platform_test.dart` 里有一条断言盯着这件事：本层一旦用上 `zonedSchedule` /
`periodicallyShow` / `showDailyAtTime` / `showWeeklyAtDayAndTime` 中任何一个，测试当场红 ——
那时候这些缺口就从「不相干」变成「真缺功能」，得先谈清楚再写。

### macOS 沙箱

I01 关掉了 App Sandbox（沙箱会挡住外壳连自己的内核）。所以 `file_selector` 不需要
`com.apple.security.files.user-selected.read-write`、`url_launcher` 不需要
`com.apple.security.network.client`、`Process.start` 也不受限。**哪天把沙箱打开，这三项要
同时补 entitlement**，否则会静默失败。

### 实测往返耗时（I01 交付时跑的，不是估算）

同一台机器、同一个 release 内核、同一个命令（`about_info`），断言写在
`test/kernel_integration_test.dart` 里，每次跑测试都会重测并打印：

| 走法 | p50 | p99 | min |
|---|---|---|---|
| 持久 Socket（本层） | **115 µs** | 378 µs | 87 µs |
| `dart:io` HttpClient | **1500 µs** | — | — |

**13.0 倍**，与 spec §1.1 的比例一致。绝对值比 spec 记的 64 µs 高 1.8 倍（机器与测法不同），
但结论不变：那 1.4 ms 是客户端抽象的开销，不是进程边界的。测试里的门槛是 p50 < 300 µs ——
哪天有人把 socket 换回 HttpClient，这条当场红。

## 决定：不生成 203 个 Dart 绑定，只有一个泛型 `invoke`

203 个命令，两条路：给每个生成一个 Dart 函数，还是只暴露 `invoke<T>(cmd, args)`。**选了后者。**

理由，按份量排：

1. **生成器本身是要维护的东西，而且这里没有现成的**。TS 侧的类型是 `ts-rs` 从 Rust
   结构体生成的（`src/services/api/types/generated/`，74 个文件带
   「generated by ts-rs. Do not edit」抬头）。ts-rs 没有 Dart 后端。要生成 Dart 绑定就得新写
   一个生成器，再让它一直跟着 Rust 走 —— 这是一整个新工具，不是一个文件。
2. **手写的绑定会静默漂移**。203 个函数签名与 Rust 侧没有任何机器校验关系。Rust 改了参数名，
   Dart 那份照样编译通过，错误推迟到运行时，且表现为「某个字段是 null」这种最难查的形态。
   一个 `invoke('cmd', {...})` 至少在后端不认时当场 400 / 404 抛出来。
3. **命令名的漂移用一条测试补回来**，不用生成整套绑定。`test/command_names_test.dart` 读
   `src-tauri/src/startup.rs` 的 `generate_handler!`（invoke 名的唯一真值源，见项目
   CLAUDE.md），把 Dart 源码里所有字面量命令名比一遍，对不上就红。这跟既有的
   `scripts/t06-handler-names.mjs` / `t08-rpc-names.mjs` 是同一个 idiom。

代价，写清楚：**字段级的类型仍然没有机器校验**。各票为自己用到的数据写 Dart model 类时，
字段名必须照抄 `src/services/api/types/generated/<Type>.ts`（那才是 Rust serde 的真值投影），
不要照着后端 struct 凭记忆写。哪天字段漂移的账真的疼了，再谈生成器 —— 那时它有真实需求撑着，
不是现在这种「以后可能要」。

## 打包与自动更新（票 I13）

**谁只认谁的产物**：Flutter 壳读 `flutter-appcast.xml`（Sparkle/WinSparkle），老 Tauri 壳读
`latest.json`（minisign）。两个 feed 在同一个 Release 上，格式互不兼容、签名密钥互不相认，
谁也不可能给对方装上包。

- macOS：Sparkle，EdDSA 公钥在 `macos/Runner/Info.plist` 的 `SUPublicEDKey`。`aidog-kernel`
  由 Xcode 的「Bundle aidog-kernel」phase 打进 `Contents/MacOS`，带自己的 bundle id
  （`<app id>.aidog-kernel`）+ hardened runtime + 独立签名（spec §5.2 的 Apple 规则）；
  本地无证书时 ad-hoc（`codesign -dv` 可自证）。
- Windows：WinSparkle，DSA 公钥是嵌入资源（`windows/runner/Runner.rc` 的 `DSAPub` ←
  `flutter/dsa_pub.pem`）。`aidog-kernel.exe` 由 CMake install 阶段落位到主程序同目录。
  安装包走 Inno Setup（`packaging/flutter_windows.iss`），WinSparkle 对 Inno 静默安装自带参数。

**版本**：`flutter/pubspec.yaml` 的 version 由 `scripts/sync-version.mjs` 从根 `.version`
同步（`+N` build number 不参与漂移判定），CI 出包时用 `--build-name/--build-number` 覆盖。

**跨栈回滚**（Flutter 版不乖 → 回 Tauri 版）：两个壳共用 `AiDog` 这个应用槽位，从 Releases
页下载上一个版本的 Tauri `*_universal.dmg` / `*_x64-setup.exe` 覆盖安装即可；装回去后它的
`latest.json` 链会继续把它留在 Tauri 线上。

**本地 mock 更新源**（验收「更新流程实跑一次」用）：

```sh
cd /tmp && mkdir -p feed && cd feed   # 放一个 appcast.xml + 一个高版本的假 zip
python3 -m http.server 5002
AIDOG_UPDATE_FEED=http://127.0.0.1:5002/appcast.xml \
  HOME=/tmp/aidog-I13-home flutter run -d macos   # 或直接开已构建的 .app
```

`AIDOG_UPDATE_FEED` 设置时启动即弹一次检查（`lib/src/updater.dart`）。

**需要 CI secrets**（本地证不了的部分）：`SPARKLE_EDDSA_PRIVATE_KEY`（Ed25519 私钥，
base64，`sign_update --ed-key-file -` 吃它）、`WINSPARKLE_DSA_PRIVATE_KEY`（`dsa_priv.pem`
全文）。私钥的生成件在 `.scratch/flutter-frontend/keys/`（gitignored），**必须备份后再设
secret** —— 丢了用户就永远升不了级。Apple 侧的真签名 / notarization 未配（与老 Tauri 壳
同水位），要配的话另需 Developer ID 证书与 notarytool 凭据。

## 内核可执行文件在哪

`resolveKernelExecutable()` 按顺序找，一条都不中就抛（不静默回落）：

1. 环境变量 `AIDOG_KERNEL_BIN`
2. 与 Flutter 可执行文件同目录 —— **发版形态**（macOS 的 `Foo.app/Contents/MacOS/`，见 spec §5.2）
3. 从当前目录逐级向上找 `src-tauri/target/{release,debug}/aidog-kernel` —— **开发形态**

开发期先建一次：

```sh
cd src-tauri && cargo build --release -p aidog_kernel
```

## 跑测试

```sh
cd flutter
flutter analyze
flutter test
```

集成测试会**真起 `aidog-kernel`**，用隔离的 `HOME=/tmp/aidog-I01-home*`，
不碰 `~/.aidog`，也不碰用户正在跑的那个 AiDog。

## 目录

```
lib/transport.dart              其余票唯一该 import 的入口
lib/src/transport/
  kernel.dart                   门面：invoke / on / states，负责重连后换地址
  kernel_process.dart           aidog-kernel 子进程：拉起、读端口、指数退避重拉
  rpc_client.dart               持久 Socket + 手拼 HTTP/1.1 + 连接池
  event_stream.dart             单连接 SSE + 按事件名扇出 + 自写重连
lib/platform.dart               原生能力 6 项（票 I12）：剪贴板 / 对话框 / 链接 / 进程 / 通知
lib/charts.dart                 图表层入口（票 I04）
lib/src/charts/
  tooltip.dart                  序列身份 + tooltip 行模型（先于任何一张图存在，零 fl_chart 依赖）
  tooltip_item.dart             行模型 → fl_chart 的文本模型
  palette.dart                  系列色 / 热力色带（色值只来自主题）
  axes.dart                     niceTicks → fl_chart 的 min/max/interval
  series.dart                   对齐降采样 / 域计算 / 图例
  line_chart.dart               折线（含双轴、面积、虚线、mini）
  stacked_area_chart.dart       堆叠面积（自己累加 + 反序绘制）
  donut_chart.dart              环形（topN + 其他）
  scatter_chart.dart            散点直方图
  heatmaps.dart                 三张热力图（格子布局，非图表库）
  gauge_chart.dart              仪表盘圆环 + 趋势 sparkline（CustomPainter）
  empty.dart                    诚实空态
lib/pages.dart                  页面层入口（票 I06）：pageBuilder 与后续页面票只 import 这个
lib/src/pages/
  invoke.dart                   页面与传输层唯一的接缝：InvokeFn + 500ms 防抖事件流
  models.dart                   页面私有的 RPC 载荷模型（字段名抄 types/generated）
  home.dart / home_logic.dart   首页（票 I06）
  stats.dart / stats_logic.dart 使用统计（票 I06）
  platforms.dart / platforms_logic.dart  平台（票 I07）
  groups.dart / groups_logic.dart        分组（票 I07，内嵌在平台页里）
  logs.dart / logs_logic.dart            请求日志两页（票 I07）
  ui_bits.dart                  票 I07 三页共用：SmallButton / ConfirmCard / CenteredNote / ToastBar
  filter_dropdown.dart          带搜索的筛选下拉
  settings/*_logic.dart         设置 12 子页的逻辑层（票 I08）
  settings/bits.dart            设置页共用表单零件（票 I16）
  settings/schema_config_page.dart  claude / codex / pi 三页共用一棵树（票 I16）
  settings/system_page.dart     系统设置（票 I16）
  settings/coding_tools_page.dart   CLI 集成（票 I16）
  settings/notifications_page.dart + notification_events.dart  系统通知（票 I16）
  settings/rules_pages.dart     调度熔断 + 中间件规则（票 I16）
  settings/mitm_page.dart       MITM 解密（票 I16）
  settings/tray_pages.dart      托盘 + 浮窗（票 I16）
  settings/importexport_page.dart   导入导出 + 定时备份 + 异源导入（票 I16）
  settings/permissions_editor.dart  claude 页权限矩阵编辑器（I17，可视化↔JSON 双模式）
  settings/hooks_editor.dart        claude 页 hooks 构建器 + notify 快捷条（I17）
  settings/middleware_editor.dart   中间件条件树 / 动作链 / 应用范围编辑器（I17）
  settings/middleware_dsl.dart      条件树 ↔ DSL 源码互转（I17，mwDsl.ts 移植）
  settings/popover_layout.dart      浮窗二维布局纯函数：normalize / moveItem / makeItem（I17）
lib/stats/aggregation.dart      Stats 二次聚合四函数（票 I05）
lib/utils/formatters.dart       数值格式化唯一落点（禁页内重复定义）
lib/src/utils/pinyin.dart        拼音模糊搜索（3500 常用字自建词典，I17）
lib/utils/color_level.dart      成功率 / 成本的色编码分级
lib/main.dart                   主窗口入口
```

## I16 定下的（2026-09-20）：设置页 widget 层

票 I08 交了 12 个子页的**逻辑层**，I16 补上 widget 树并接进 `main.dart` 的 `pageBuilder`。
**逻辑一行没重写** —— 在 widget 里再写一遍校验就是第二份真值源，两份一定会漂移。

接进 `pageBuilder` 的 12 个 id：`settings`（裸 id 回退 system）/ `settings/system` /
`settings/coding_tools` / `settings/claude` / `settings/codex` / `settings/pi` /
`settings/middleware` / `settings/scheduling` / `settings/notifications` /
`settings/tray` / `settings/popover` / `settings/importexport` / `settings/mitm`。
**`settings/pricing`（模型信息）不在本票**：它没有 I08 的逻辑层，属票 I09 的
`src/pages/ModelInfo/`，那一页落地前仍走占位。

### 离页守卫挂在哪

| 页 | 有守卫 | 理由 |
|---|---|---|
| `settings/claude` | ✅ 三个出口（保存并离开 / 放弃 / 取消） | React `Settings.tsx:340` 就是这样 |
| `settings/codex`、`settings/pi` | ❌ | **照搬 React**：它俩没有守卫也没有 Cmd+S，改属产品改动，单开票 |
| `settings/coding_tools` | ✅ 代理两个输入框有草稿时 | **相对 React 的有意增强**，见下 |
| `settings/middleware` | ✅ 规则表单开着时 | **相对 React 的有意增强**，见下 |
| 其余 8 页 | ❌ | 即时保存，没有草稿态，不存在「静默丢编辑」 |

两处增强的理由：React 的这两个草稿分别靠 `onBlur` 与 modal 收口，而桌面壳里点侧栏
会直接把整棵页面树拆掉 —— `onBlur` 不一定触发、modal 不存在，编辑会**静默丢失**。
守卫只在真有草稿时挂，不脏就注销，对 React 行为的唯一可见差别是「点走时会先问一句」。

### 与 React 的未对齐（照实列，不是没想到）

> I17 已关闭本节原有四项：拼音搜索、claude 权限/hooks 专用编辑器、中间件条件树/动作链/DSL 编辑器、托盘与浮窗拖拽排序、浮窗二维栅格预览。实现与测试见对应 I17 提交。

### 顺手修掉的一处存量 bug

`settings/importexport_logic.dart` 里 `scope|key` 复合键的分隔符是一个**字面 NUL 字节**
（`'${e['scope']}\x00${e['key']}'`，join 与 `_splitKey` 三处一致，所以功能是对的）。
NUL 在编辑器里不可见、会让 grep / diff / 测试里的字符串字面量全部对不上 —— 本票把三处
一起换成普通空格，行为不变，I08 的断言照常通过。

### 命令覆盖

本票的 widget **没有引入逻辑层之外的新命令**，所以
`test/settings/command_coverage_test.dart` 的 96/96 零差集继续成立，数字没动。
