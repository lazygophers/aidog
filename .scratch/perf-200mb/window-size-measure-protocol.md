# 窗口尺寸-内存量测协议

Task: `window-default-size` / subtask `s1-measure-protocol`

复用 `.scratch/perf-200mb/measure-protocol.md`（`frontend-compositing-purge` 沉淀）的判据与工具，
针对本 task 的 4 档窗口尺寸曲线量测补硬约束。**本文档只定协议，不跑量测**（协议见验收标准 1）。

## 前置依赖

窗口面积必须在 `frontend-compositing-purge` 的前端合成层清理**完成之后**测，否则量的是
被污染的基线（数字对不上 [03] 的 dev/release 拟合式，也对不上本 task 想定死的「可信系数」）。
本协议假定该 task 已 finish；若未 finish，先确认再开量测窗口。

## 构建

```bash
yarn tauri build --bundles app  # release 构建 + 打 .app 包，产物在 src-tauri/target/release/bundle/macos/AiDog.app
```

⚠️ **禁用 `--no-bundle`** —— 它只产裸二进制 `src-tauri/target/release/aidog`，**不产 `.app`**，
`bundle/macos/` 目录根本不会创建。本文档初版写的 `--no-bundle` + `bundle/macos/AiDog.app`
是自相矛盾的，run1 前的构建栽在这里，只能补跑 `--bundles app`（复用编译产物，只做打包）。
打包时报 `A public key has been found, but no private key ... TAURI_SIGNING_PRIVATE_KEY`
只影响 updater tar.gz 签名，`.app` 已正常产出，可忽略。

**禁用 dev 数据外推**——[03] 已证 dev 拟合常数项（16.7MB）与 release（67.3MB）差 50MB，
不可比。若 `/Applications/AiDog.app` 不是本次 release 构建产物，先手工替换
（`cp -R src-tauri/target/release/bundle/macos/AiDog.app /Applications/`），
采样前用 `.app` 的 mtime 核验（复用下方自证第 3 项）。

## 硬约束（继承 measure-protocol.md，逐条对齐本轮）

1. **release 构建，非 dev**——构建命令见上。
2. **每尺寸独立重启 app**，禁同进程内 `osascript` 改窗口尺寸后连续采样
   （[03] 栽点：同进程内改尺寸时主进程/GPU 仍在大幅波动，两点不在同一稳态，拟合系数不可信）。
   每档流程：`pkill -x aidog` → 等 5s → 用 `osascript` 设置窗口 bounds 启动 → 等满稳态 → 采样 → 结束该档。
3. **等满 ≥10 分钟稳态**才采样（冷启动 50MB@25s → 稳态 149MB@22min 的爬升曲线已实证，见
   `measure-protocol.md` 第 6 条）。
4. **判据只认 `footprint` 命令的 `phys_footprint`（全进程 TOTAL）与 graphics 分类行
   （`Owned physical footprint (unmapped) (graphics)`）**；**禁 `ps rss`**（漏算压缩内存/swap）
   **禁 `vmmap`**（同样漏算 graphics 分类，[01] 已证）。复用 `assets/measure.sh mem <label>`，
   该脚本已按此口径实现（`fp_mb()` 解析 `phys_footprint:`，footprint 全量输出留档供 graphics
   行提取）。
5. **region 数不作判据**——[03] 实测同一份 CSS 下 157/211/218 乱跳，纯噪声。
6. **背景态口径（run2 后二次修订，2026-07-29 用户拍板）**：**内存量测全程走背景态，禁 `activate`**。
   launch + 设尺寸后主动 `tell application "Finder" to activate` 把 AiDog 推到背景，
   此后 600s 稳态期用户怎么用电脑都不影响读数，采样时前台是谁不作判据。
   - 修订理由：run1（判据=全程不切换）与 run2（判据=activate 后 90s settle）**各 4 档全废**，
     栽点相同——AiDog `activate` 后只能保住前台 10–20s 就被 Arc / Warp / 飞书抢走，
     「让 AiDog 独占前台 N 秒」在人正常用电脑时不可执行。
   - 而**用户已拍板「量测验收口径 = 只认背景态可比读数」**，activate + settle 是从 **CPU 量测**
     继承来的（CPU 需前台态才有代表性），对内存量测无必要。删掉后判据天然满足。
   - CPU 量测仍需前台态 + 90s settle（30s 已证伪，见
     `.skein/spec/recall/optimization/measure-window-multi-probe.md`），本条只对内存量测放宽。

7. **进程编制核验（run2 后新增硬闸）**：`measure.sh launch` 的差集口径
   （launch 前后各拍一次全系统 WebKit XPC pid 集合，差集即本 app 的）有固有缺陷——
   **窗口期内其他 WKWebView 宿主（飞书 / 微信 / Safari）新起的 helper 会被误纳**。
   run2 实证：档 3 多出 1 个 WebContent（pid 54276, 106MB），档 2 / 档 4 的 GPU 分别虚高到
   109MB / 95MB（正常 21–24MB）。
   - 归属反查不可行：WebKit helper 的 `ppid` 恒为 1（launchd），`ps -o args=` 三个 app 完全相同，
     `launchctl procinfo <pid>` 需 root。
   - 改用**编制上限做硬闸**：AiDog 恒为 **WebContent×2**（主窗口 + `prebuild_popover` 预建窗口）
     **+ GPU×1 + Networking×1**。差集结果不等于这个编制 → `launch` 退出码 2，该档重取（重试 ≤3 次）。

## 自证四项（每档采样必须全部落盘）

1. 采样时间戳
2. `.pids` 内进程**采样时仍全部存活**（run1 档 1 栽点：5 个 pid 全退出，TOTAL=0——
   刚 `cp` 进 `/Applications` 的 ad-hoc 签名 app 首次运行时 macOS 做校验并杀进程，
   同时 touch 掉 `.app` 的 mtime。**对策：正式量测前先跑一次弃用的 warm-up 启动**，
   让首次校验在量测窗口外完成）
3. `/Applications/AiDog.app` 的 mtime 是否落在本档 `launch` 之前
4. **进程编制核验 PASS**（`measure.sh launch` 输出 `编制核验 PASS: WebContent=2 GPU=1 Networking=1`），
   并记该档 `launch` 尝试次数。采样时前台是谁只留档不作判据（背景态口径，非 AiDog 才是预期）

缺一项，该档数据作废。

## 尺寸档与窗口设置

至少 4 档，含 `1026×759`（新默认）与 `2304×1265`（当前 `maximized:true` 在本机屏幕的对照）：

```
1026×759   （新默认，非最大化）
1150×750   （[03] release 已有一点，用于交叉核验）
1800×1100  （中间档）
2304×1265  （当前 maximized 对照，本机屏幕分辨率）
```

启动后设置窗口尺寸（launch 与设尺寸在同一次进程生命周期内做，不算「同进程内改尺寸后连续采样」——
后者指的是「同一进程内测完一档又改尺寸测下一档」）：

```bash
osascript -e 'tell application "System Events" to tell process "AiDog" to set position of window 1 to {100, 100}'
osascript -e 'tell application "System Events" to tell process "AiDog" to set size of window 1 to {1026, 759}'
```

## 复现步骤（每档独立执行）

```bash
DIR=.scratch/perf-200mb/assets
W=1026; H=759; LABEL=w1026x759

pkill -x aidog 2>/dev/null; sleep 5
$DIR/measure.sh launch || echo "编制核验超编 → 本档重取"   # 退出码 2 = 超编，重试 ≤3 次
osascript -e "tell application \"System Events\" to tell process \"AiDog\" to set size of window 1 to {$W, $H}"

# 推到背景：内存量测口径 = 背景态。让 Finder 抢走前台，此后用户怎么用电脑都不影响读数。
osascript -e 'tell application "Finder" to activate'

sleep 600          # ≥10min 稳态门槛。无 activate、无 settle、无 regime 探针。
$DIR/measure.sh mem "$LABEL"

# 自证核验
stat -f "%Sm" /Applications/AiDog.app               # 项3，须早于本档 launch 时间戳
cat "$DIR/.pids"                                     # 项2，与下一档对比确认不同实例
# 项4 = launch 输出里的「编制核验 PASS」行 + 尝试次数
```

对 4 档重复上述块，`W`/`H`/`LABEL` 换档。

## 产出

- 曲线表：4 档的 `graphics` 字节 + 全进程 `TOTAL phys_footprint`
- release 口径线性拟合式（对齐 [03] 的 `graphics(MB) ≈ a × 面积 + b` 形式）
- 与 dev 拟合式（`7.35e-5 × 面积 + 16.7`）及 [03] 不干净的 release 两点拟合
  （`6.34e-5 × 面积 + 67.3`）的差异说明
- 默认尺寸（1026×759）下是否 ≤200MB 的明确结论

## 清场

量测完成后，`assets/measure.sh` 与 `loadgen.sh` 属**保留态工具**（PRD 边界外的可复用资产），
不删；但本 task 自己产出的逐次采样中间文件（`results/mem-w*.txt`、`results/regime-w*.log`
等原始记录）在最终曲线表写入 task 产物后按 PRD 验收标准「清场完成」删除，只留最终曲线表。
run1/run2 的作废原始数据（`results/size-curve-run*-VOID.txt`、`regime-w*.log`、`climb-w*.log`）
同批删除——修订理由已写进本协议正文，无须留原始盘。
