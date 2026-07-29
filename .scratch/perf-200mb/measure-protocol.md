# 前端合成层性能量测协议

Task: `frontend-compositing-purge` / subtask `s1-measure-protocol`

复用并加固了 `票01`（`.scratch/perf-200mb/issues/01-measure-baseline-and-attribute-memory.md`）
已验证的口径，新增一条本轮实测发现的硬约束（窗口焦点/可见性）。

## 判据硬约束

1. **内存只认 `footprint -p <pid>`** 的 `phys_footprint` 与其分类明细（`WebKit malloc` /
   `Owned physical footprint (unmapped) (graphics)` 两个分类行直接给出 metric ①④，
   不需要额外解析）。**禁 `ps rss`**（漏算 graphics 类目）**禁 `vmmap`**（同样漏算，已在票01实证）。
2. **堆块数交叉核验用 `heap <pid>`**：`grep -E "^All zones: [0-9]+ nodes \("` 取
   `All zones: N nodes (BYTES bytes)` 一行作为该进程 malloc 总字节的独立信源，
   与 footprint 的 `WebKit malloc` 分类行做交叉验证（本轮实测两者一致，偏差 <8%，见下）。
   heap 的详细块大小直方图（`Sizes: ... 16KB[n] ... 32B[n] ...` 一行）留作后续按块大小
   （如 ≤5KB 小块 vs 大块）细分诊断用，本轮基线不需要。
3. **`region 数是噪声指标`**（票03 已证：同一份 CSS 下 157/211/218 乱跳）——
   **只记字节数，不记 region 数**。
4. **CPU 只认 `ps -o time=` 区间累计差值 / 墙钟**，不用 `ps %cpu`（生命周期均值测不出当下负载）。
5. **每档独立重启**：不同配置（改前/改后 CSS）之间必须 `pkill -x aidog` 后完全重新
   `open -a`，禁止在同一进程内切 CSS 做 A/B（票03 已证：首轮就因 GPU 进程未吃满而作废，
   本轮 run3 又反证——同进程内窗口焦点状态漂移会让同一份 CSS 测出 0%~58% 的 CPU）。
6. **等满稳态 ≥10 分钟**：启动后立即采样不可信（内存有冷启动 50MB@25s → 稳态
   ~150MB@22min 的爬升曲线，见 `results/main-growth-curve.txt`）。
7. **量测设施全局单例，同一时刻只允许一个 subtask 持有**（2026-07-29 s3/s5 互踩后加）：
   `/Applications/AiDog.app`（单实例）、`assets/measure.sh`、`assets/.pids` 三者共享，
   且 `pkill -x aidog` 是全局动作 —— 两个 subtask 并发跑各自的「前后对比」，会互相
   pkill/relaunch 导致 pid 无预警更迭，两组数据同时作废且**事后难以判定谁污染了谁**
   （实测：s3 与 s5 的采样 pid 完全重合 55942/13817）。
   **持有规则**：进量测周期前先向 coordinator 报备取得独占窗口，窗口内其他 subtask
   禁 `cargo` / `yarn build` / `yarn tauri` / `pkill` / 碰 `.app` 与 `.pids`（编译争
   target 锁 + 打满 CPU 同样污染 idle 读数）。窗口交还前不得开始下一个。
   **采样必带三项自证**：① 采样时间戳 ② before/after 两端 pid 是否同一实例
   ③ `/Applications/AiDog.app` 的 mtime 是否落在 before 采样之前。三项缺一 = 该组作废。

## 本轮新增发现：窗口焦点/可见性是未受控变量，必须显式核验

**现象**：两次「同样重启 + 等满10分钟 + 立即采样」的独立跑，CPU 从 58.3% 跌到 7.6%，
偏差 653%，远超 10% 阈值。

**根因定位**（`osascript -e 'tell application "AiDog" to activate'` 前后对照）：

| 状态 | 全进程 CPU% | WebContent footprint |
|---|---|---|
| 窗口存在但未真正前台激活 | 7.6% ~ 0% | 295~446MB |
| 显式 activate 后立即采 | 57.0% | — |
| activate + settle 30~90s | 54.2% | 493~744MB |
| 与「稳态时长」无关的对照组（run1，恰好处于前台态） | 58.3% | 491MB |

这与票01已记录的「窗口隐藏 → 0.2%」是同一现象的连续谱，不是新矛盾：
**只要窗口不是真正的前台可见+激活态，CPU 与合成层内存都会显著走低**，
且这个状态可以在「窗口未最小化」的情况下悄悄发生（本机会话下 `System Events`
的 `frontmost` 查询本身也不可靠——见下"已知环境限制"）。

**协议修正**：

- 采样前必须执行 `osascript -e 'tell application "AiDog" to activate'`，
  并在 activate 之后**再等待一段 settle 时间**（本轮用 30~90s 生效，
  activate 瞬间会触发一次性重绘/重新合成的短暂尖峰，不能在 activate 后
  立即采——尖峰期 WebContent graphics 可达 637~744MB，比真实稳态高 30~90%）。
- 采样前后建议用 `osascript ... frontmost` 或截图核验窗口真的在前台；
  **已知环境限制**：本次执行环境里 `System Events` 的 frontmost 查询
  持续返回宿主终端进程名而非真实前台应用，`screencapture` 也因缺 Screen
  Recording 权限直接报错 `could not create image from display`
  （与票01"screencapture -R 亦无屏幕录制权限"记录一致）。**这意味着
  本 agent 会话内无法 100% 程序化验证窗口前台状态**，只能用 CPU/内存
  读数本身是否落在"前台态数量级"（CPU ≥40%、WebContent footprint
  ≥400MB）反推是否采样有效——若读数明显落入"隐藏/背景态数量级"
  （CPU <10%、footprint <350MB），该次采样作废重采，不得计入基线。
- **需要**: 若后续 subtask 需要更强的前台状态保证，建议由能操作真实
  显示会话的人工在采样窗口内保持 AiDog 窗口聚焦，或在有 Screen Recording
  权限的环境下跑本协议。

## 🔴 判据改判（2026-07-29，用户拍板）：只认背景态可比读数

上面「落背景态即作废重采」的规则**已废止**。实际执行里 fe-s3 与 fe-s5 各自跑到
第 2 轮 / 第 6 轮仍无法稳定落进前台态 —— 根因是环境墙（无 Screen Recording 权限
+ `System Events` frontmost 不可靠），不是采样手法问题，重采再多轮也是撞同一堵墙。

**新判据**：

1. **统一在背景态下做 before/after 对比**，不再追求前台态。前台态读数可遇不可求，
   不作为验收依据。
2. **两端 regime 必须同深度才可比** —— 这是新判据下唯一的有效性门槛。
   反面教材：s5 的 `before`（WebContent footprint 362MB）与 `after-clean`（111MB）
   虽都低于 350MB 门槛，但深度差 3 倍，-66% 的总量降幅主要来自 graphics
   随失焦释放，**不能归因给 CSS 改动**。可比性判据：两端 WebContent footprint
   同一数量级（差值 <30%），否则该组作废。
3. **原始目标口径的代价（用户已知悉并接受）** —— 「实际使用中 ≤200MB / CPU <0.5%」
   这个目标在背景态下无法直接验证，背景态读数天然低于真实使用态。本轮验收
   是**相对口径**（改动前后的降幅），不是绝对口径。
4. **合成层类改动（backdrop-filter / 动画 / glass buffer）在背景态基本测不出** ——
   graphics 内存已随失焦释放，省的正是这块。这类 subtask 不再逐个量测，
   统一移交 `perf-final-verification` 做一次加总对比。

## 复现步骤（可直接照抄）

```bash
DIR=.scratch/perf-200mb/assets

# 1. 独立重启
pkill -x aidog; sleep 5
$DIR/measure.sh launch          # 输出 .pids，含 main + 4 个 WebKit XPC pid

# 2. 立即 activate 一次（部分环境下有效，见上），随后等满稳态
osascript -e 'tell application "AiDog" to activate' 2>/dev/null
sleep 600                       # ≥10 分钟稳态

# 3. 采样前再 activate + settle，防止 activate 尖峰污染读数
osascript -e 'tell application "AiDog" to activate' 2>/dev/null
sleep 30

# 4. 内存分解（footprint，含 WebKit malloc / graphics 两个分类行）
$DIR/measure.sh mem <label>

# 5. CPU（30s 区间累计差值）
$DIR/measure.sh cpu <label> 30

# 6. 交叉核验 malloc（可选，诊断用）
heap <WebContent-pid> | grep -E "^All zones: [0-9]+ nodes \("

# 7. 读数校验：若 CPU <10% 或 WebContent footprint <350MB，判定未真正前台，作废重采
```

## 四项基线指标定义

| # | 指标 | 取数方式 |
|---|---|---|
| ① | WebContent 进程 WebKit malloc | 主窗口 WebContent 的 `footprint -p` 输出中 `WebKit malloc` 分类行 |
| ② | 空闲前台 CPU | 全进程（main+GPU+Networking+WebContent×N）`cpu` 子命令 TOTAL 行 |
| ③ | 全进程 phys_footprint 总和 | `mem` 子命令 TOTAL 行 |
| ④ | GPU helper graphics 字节 | GPU 进程 `footprint -p` 输出中 `Owned physical footprint (unmapped) (graphics)` 分类行 |

## 验收对照（两次连跑偏差）

见 `results/baseline-frontend-compositing.md`。
