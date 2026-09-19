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
lib/main.dart                   占位外壳，票 I02 会整个换掉
```
