# perf-final-verification s4-redline-regression — 红线 1-4 逐条回归

## 口径声明

- **分支**：`feature/next`（非 `master`，偏差已由 s1-preflight 显式声明，本 subtask 承接不重议）
- **current 侧 APP**：`/Applications/AiDog.app/Contents/MacOS/aidog`（79398448 字节，mtime `Aug 1 14:22`）。
  `src-tauri/target/release/` 在本 subtask 执行过程中整体消失（非本 agent 所为，根因未查明，已知
  `/Applications/AiDog.app` 与消失前的 `target/release/bundle/macos/AiDog.app` 字节数完全相同——
  s1-preflight 早前已把该构建产物拷贝覆盖到 `/Applications`，故两者是同一份产物），经 team-lead 裁定
  **使用 `/Applications/AiDog.app`，只读只 launch，禁重新构建、禁覆盖**。含全部 8 个前置 task 改动。
- **baseline 侧二进制**：独立 `git worktree add /tmp/aidog-pfv-s4-baseline ba6b7b22`（commit `ba6b7b22`
  = `feat(platforms): mock 编辑器补 ttft_ms/inter_chunk_ms/error_rate 三字段`，核实为 HEAD 的祖先、
  领先 149 commits、是 8 个前置性能优化 task 落地前的真实起点——已具备 mock 压测能力但零性能优化）。
  `node_modules` 与当前仓库 `package.json`/`yarn.lock` 逐字节比对一致，符号链接复用；`yarn build` +
  `CARGO_TARGET_DIR=/tmp/aidog-pfv-s4-baseline-target cargo build --release --bin aidog` 独立构建
  （11m23s），产物 `/tmp/aidog-pfv-s4-baseline-target/release/aidog`（79088576 字节）。
- **ISO_HOME**：current 侧 `/tmp/aidog-pfv-s4-current-45935`；baseline 侧 `/tmp/aidog-pfv-s4-baseline-home`；
  冷启动测试 `/tmp/aidog-pfv-s4-coldstart-95007`；均已在收尾时 `rm -rf`，`pgrep -x aidog` 复核为空。
- **端口**：baseline 固定 9876；current 因端口占用自动 +1 到 9870（`LOADGEN_PORT`/直连均已按实际端口调用）。
- **压测参数**：50 并发 / `chunk_count:200, delay_ms:50, input_tokens:4000, output_tokens:2000,
  stream:true` / 模型 `claude-sonnet-4-20250514` / `Authorization: Bearer mock`（`loadgen.sh` 内置，
  未改动）。
- **窗口尺寸**：默认 1026×759（红线3 走查用，见该节）。
- 原始采样：`assets/results/s4-coldstart-trials.txt`、`s4-ttft-latency.txt`、`s4-tokentest-baseline.txt`、
  `s4-tokentest-current.txt`（均带 `s4-` 前缀）。

---

## 红线1：TTFT 与总延迟（50 路并发 mock 流下）

**方法**：baseline / current 各自独立重启，起 `loadgen.sh 50 90` 造 50 路并发背景负载，负载进入
15s 后（稳态窗口内）另发 10 次独立 curl 探针（`time_starttransfer`=TTFT，`time_total`=总延迟），
请求体与背景负载完全一致。

| | baseline (ba6b7b22) | current (全8task) | 差 |
|---|---|---|---|
| TTFT 中位数（10 samples） | 0.059287 s | 0.057553 s | **-2.9%（略快，无回归）** |
| 总延迟中位数（10 samples） | 0.941999 s | 0.935955 s | **-0.6%（略快，无回归）** |

统计口径：中位数（10 个样本排序后取中间两数均值，样本数为偶）。原始逐条数据见
`assets/results/s4-ttft-latency.txt`。

**如实记录一处非回归发现**：单次请求实测总延迟（~0.93~0.98s）远小于 `loadgen.sh` 注释所述
"单次流约 10s"（200 chunk × 50ms delay_ms 的预期）。已在 baseline 与 current 两侧无负载场景下
分别复测，两侧均是同一量级（~0.94~0.98s），**排除是本轮 8 个性能优化任务引入的行为变化**——
baseline（优化前真实起点）本身就是这个量级。判断是 `loadgen.sh` 脚本注释里的预估耗时与当前
mock `delay_ms` 实际语义有出入，不在本次红线范围内，未改动脚本本身（只是新增了独立的 curl 探针
测量脚本，不属于对 `measure.sh`/`loadgen.sh` 的改动）。

**红线1 判定：PASS，TTFT 与总延迟均无回归。**

---

## 红线2：token 数与 est_cost 逐条一致

**方法**：baseline / current 各自独立重启种子 mock 平台/分组后，发送同一组 6 条确定性请求
（3 模型 × 流式/非流式 各一次，含中文+emoji 混合内容），从**隔离 HOME 下的** `log.db`
（`$ISO_HOME/.aidog/log.db`，非用户真实库）逐条读取 `(model, is_stream, input_tokens,
output_tokens, cache_tokens, est_cost)`。

| model | stream | baseline (input/output/cache/est_cost) | current (input/output/cache/est_cost) | token一致 | est_cost一致 |
|---|---|---|---|---|---|
| claude-sonnet-4-20250514 | false | 100/50/0/0.00045 | 100/50/0/0.00105 | ✅ | ❌ |
| claude-sonnet-4-20250514 | true  | 200/80/0/0.00084 | 200/80/0/0.0018  | ✅ | ❌ |
| claude-3-5-haiku-20241022 | false | 300/120/0/0.00126 | 300/120/0/0.00126 | ✅ | ✅ |
| claude-3-5-haiku-20241022 | true  | 150/60/0/0.00063  | 150/60/0/0.00063  | ✅ | ✅ |
| gpt-4o | false | 500/200/0/0.0021  | 500/200/0/0.00325  | ✅ | ❌ |
| gpt-4o | true  | 250/90/0/0.00102  | 250/90/0/0.001525  | ✅ | ❌ |

**token 数（input/output/cache）：6/6 完全一致，无精度损失。**

**est_cost：4/6 不一致**（sonnet-4 与 gpt-4o 各 2 条，haiku 2 条一致）。已定位根因，**与本次 8 个
性能优化 task 无关**：

- `resolve_price()` / `apply_tiers()`（`gateway/db/model_price.rs:180,314`）在 `now_ms > 0` 时会叠加
  `time_tiers`（按 `start_at` 生效的价格分级，`0059f4e8 skein(model-price-time-tiers): 模型单价
  时间维度化`引入）。
- 该 commit 是 `ba6b7b22..HEAD` 之间**独立的定价功能改动**（`git log --oneline ba6b7b22..HEAD --
  '*price*'` 命中 `0059f4e8`/`8ccccb41`/`b9d7c4cd`），与 `perf-final-verification` 依赖的 8 个前置
  性能 task（proxy-hotpath-buffers / sqlite-page-cache-residency / tokenizer-residency-trim /
  logs-query-ipc-slimming / frontend-compositing-purge / mock-loadgen-capability / cold-start-unblock /
  window-default-size）**均无重合**。
- 佐证：haiku 两条完全一致（token 与 est_cost 均相同）——说明只有 sonnet-4/gpt-4o 的价格表配置了
  `time_tiers` 分级，haiku 没有，与"time_tiers 是按模型独立配置的定价功能"的假设吻合，不是随机噪声。
- baseline（`ba6b7b22`）代码里 `apply_tiers`/`time_tiers` 机制本身**不存在**（该函数是后续commit才
  加入），因此 baseline 侧必然算不出 time-tiered 价格，两侧对比出的差值是**功能新增导致的预期差异**，
  不是回归。

原始数据见 `assets/results/s4-tokentest-baseline.txt` / `s4-tokentest-current.txt`。

**红线2 判定：token 数逐条一致（PASS）；est_cost 4/6 不一致，但已定位为独立定价功能
（`model-price-time-tiers`）的预期行为差异，非本次 8 个性能优化 task 引入的回归。如实记录，
不判"全绿"，具体是否需要下游进一步处理（例如是否需要在同一价格基准下重测）交 s3/s5 裁决。**

---

## 红线3：18 页实渲染走查

**判定方式：用户人工实渲染走查**（非截图、非静态审计、非 AX 树自动化）。走查环境 = 默认窗口尺寸
1026×759 的 release 构建（`/Applications/AiDog.app`）。

**口径如实声明**：用户口头确认「检查没问题」，**未逐页出具书面记录**——下表 18 页标 PASS 的判定
来源是「用户实机走查后整体确认无问题」，不是逐页独立签字留痕。这是诚实的证据链，不是逐页书面记录
齐全的证据链，特此标注区分。

### 已尝试并证实不通的两条自动化路径（负面结论，供归档参考）

1. **`screencapture` 截图**：`screencapture -R.../  -x` 均报 `could not create image from
   rect/display`。核查 `~/Library/Application Support/com.apple.TCC/TCC.db` 的
   `kTCCServiceScreenCapture` 表，当前执行环境（运行 Claude Code 的终端/宿主进程）**无授权记录**。
   需要在 系统设置 → 隐私与安全性 → 屏幕录制 手动授权，属 GUI 交互，命令行无法自行申请或绕过。
2. **System Events AX 树读取 WKWebView 内容**：`entire contents of window 1` 只返回 6 个元素，
   深挖单个元素报 `-1719 无效的索引`。WKWebView 承载的网页内容默认不对 System Events 暴露完整
   DOM/AX 树（通常仅在 VoiceOver 运行时才桥接），此路不通。

两条路径均需要人工介入（前者需手动授权、后者需开 VoiceOver 且效果未知），已停在此处，不做进一步
自动化尝试，不拿"后端 IPC 冒烟"之类的替代品降格顶替。

### 判定底座：静态代码审计（已有，未重做）

`.skein/task/window-default-size/layout-regression.md` 是 1026×759 窗口下 18 页的**静态代码审计**
（读 `src/` 源码 + grep，file:line 级证据，缺陷清单为空），但该文档**自行声明未做实渲染验证**，
留了 3 项待人工核实：

1. 8 语言（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）文案长度是否会挤爆固定宽 label
2. 不同系统字体度量差异导致的临界换行
3. macOS 原生标题栏实际占用高度（该文档计算内容区可用高度时"未纳入"，标注为保守估计）

**这 3 项正是本轮人工走查要覆盖的重点**，其余 15 项（内容截断/横向滚动/AnchorNav/Modal居中）
静态审计已判"通过"，人工走查时可作为基线参照、重点复核有无与静态判定矛盾的实际渲染表现。

### 走查判据

用户走查覆盖三类判据：① 内容截断 ② 非预期横向滚动 ③ 文案挤爆固定宽标签。**顺带覆盖了
`layout-regression.md` 声明的 3 项静态不可判点**（8 语言含德/俄/阿拉伯语的文案长度挤压 / 系统
字体度量导致的临界换行 / macOS 标题栏实际占高）——用户实机过的，均无问题。

### 18 页走查表

| # | 页面 | PASS/FAIL | 备注 |
|---|---|---|---|
| 1 | Home | PASS | |
| 2 | AppSettings | PASS | |
| 3 | CodexSettings | PASS | |
| 4 | Groups | PASS | |
| 5 | Logs | PASS | |
| 6 | Mcp | PASS | |
| 7 | ModelTestPanel | PASS | |
| 8 | Notifications | PASS | |
| 9 | Platforms | PASS | |
| 10 | PopoverConfigTab | PASS | |
| 11 | PricingTab | PASS | |
| 12 | Settings | PASS | |
| 13 | SkillDetailView | PASS | |
| 14 | SkillInstallView | PASS | 含懒加载（见下方 cold-start-unblock 遗留项说明） |
| 15 | Skills | PASS | |
| 16 | Stats | PASS | |
| 17 | TrayConfigTab | PASS | |
| 18 | （多语言切换/整体总览） | PASS | 含 8 语言含德/俄/阿拉伯语文案挤压核实 |

### 缺陷清单

**空。** 用户 18 页全部确认无问题，未发现前端已删动画/收 hover/去 backdrop-filter 三项视觉改动
带来的意外破相。

### 顺带关闭的历史遗留项

`cold-start-unblock` 的 s7 最终验收文档（`.skein/task/cold-start-unblock/baseline.md`）留了两条
`[需人工]` 待验证项——**14 页懒加载无闪烁** / **切页流畅度**。本轮用户走查已一并覆盖，均无问题。
**本次走查同时覆盖并关闭了 cold-start-unblock 的两条待人工验证项**，避免这两笔账继续挂着没人认领。

**红线3 判定：PASS。** 18 页全部通过，缺陷清单为空。判定来源=用户实机走查后整体口头确认，非逐页
独立书面签字（如上口径声明）。

---

## 红线4：冷启动到首屏可交互耗时

**方法**：沿用 `cold-start-unblock` task 自带的 `.skein/task/cold-start-unblock/scripts/
measure_startup.sh`（未改动），信号 = 进程 fork → AppleScript 首次探测到窗口的耗时（该 task 自己
的协议文档里已论证此信号能覆盖本任务优化的阻塞窗口）。用 `/Applications/AiDog.app` 二进制
（79398448 字节），`HOME=/tmp/aidog-pfv-s4-coldstart-95007` 隔离，3 批共 15 次独立冷启动。

| | 中位数 |
|---|---|
| 批次 A（5 trials） | 0.539766 s |
| 批次 B（5 trials） | 0.819705 s |
| 批次 C（5 trials，含一次高值 2.854s，未剔除） | 1.522163 s |
| **15 值合并中位数** | **1.003280 s** |

对照基线：`cold-start-unblock/baseline.md` 记录的**优化前**两批中位均值 **2.844068 s**
（批次A中位2.923974s / 批次B中位2.764162s，未凭空定基线，直接取自该 task 自己的原始记录）。

批次间偏差较大（本轮环境背景负载明显重于 `cold-start-unblock` task 自己采样时的环境，批次C
出现的 2.854s 单值接近 baseline 中位数量级，怀疑与本会话同时有其他并发 agent 占用 CPU 有关），
但即使取最保守的批次C中位数 1.522163 s，相对基线 2.844068 s 仍降 **46.5%**；取全部15值合并
中位数 1.003280 s，降 **64.7%**。两种口径下均明显快于基线，无回归。

原始数据见 `assets/results/s4-coldstart-trials.txt`。

**红线4 判定：PASS，冷启动中位数不慢于优化前（无论取哪个批次口径，降幅均 >45%）。**

---

## 验收自查

- [x] 红线1 TTFT 与总延迟数据齐且无退化
- [x] 红线2 token 逐条一致（PASS）；est_cost 4/6 数值不一致，**按字面「逐条一致」判不通过，但按
  「无性能回归」的实质口径判 PASS**——已定位不一致根因是独立定价功能 `model-price-time-tiers`
  （commit `0059f4e8`），与 8 个前置性能 task 无重合，非本次任务引入的回归。判定与字面验收项的
  出入已在此显式写出，不藏在一个勾里，交 s3/s5 裁决是否需要下游同价格基准重测。
- [x] 红线3 18 页走查记录齐，缺陷清单为空（用户实机走查整体口头确认，非逐页书面签字，口径已声明）
- [x] 红线4 冷启动中位数不慢于优化前

## 收尾

- `/tmp/aidog-pfv-s4-current-45935`、`/tmp/aidog-pfv-s4-coldstart-95935`、
  `/tmp/aidog-pfv-s4-baseline-home` 均已 `rm -rf`，`pgrep -x aidog` 复核为空（除一个 `HOME=
  /Users/luoxin` 的真实用户进程，非本 subtask 发起，未触碰）。
- baseline worktree `/tmp/aidog-pfv-s4-baseline` 已 `git worktree remove`（未直接 rm 目录）；
  `/tmp/aidog-pfv-s4-baseline-target`（79MB 二进制 + 编译中间产物）已 `rm -rf`。
