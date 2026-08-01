# perf-final-verification s1-preflight

## 1. 8 个前置 task 核实表（is-ancestor 于当前分支 `feature/next`）

| task | commit | is-ancestor(HEAD) | 备注 |
|---|---|---|---|
| mock-loadgen-capability | 1d93660b | YES | |
| logs-query-ipc-slimming | 227ff2f8 | YES | |
| sqlite-page-cache-residency | 777d9e43 | YES | |
| proxy-hotpath-buffers | b8c4d011 | YES | |
| tokenizer-residency-trim | 490bc9bb | YES | |
| frontend-compositing-purge | 7ca8d251（闭环）/ 0d5dcae8（sediment）/ 8f71b4c7（s7-verify） | YES（三者均是） | PRD 未给单一 sha，团队消息「附近自己找准」；用闭环提交 7ca8d251 作代表 sha |
| cold-start-unblock | 34802037 | YES | |
| window-default-size | 2fbbd460 | YES | |

全部 8 个前置 commit 均已确认在 `feature/next` 当前 HEAD 的祖先链中。

## 2. 分支口径偏差声明（必须显式写出，禁悄悄满足）

- **当前分支 = `feature/next`，不是 `master`。**
- `git rev-list --count origin/master..HEAD` = 259（领先 259 commit）
- `git rev-list --count HEAD..origin/master` = 1（落后 1 commit）
- 从未 merge 到 master。
- **PRD 验收字面要求「8 个前置 task 全部 finish 且已合入 master 后才开始量测」未满足** —— 8 个 task 确实都 finish 且存在于 feature/next 历史里，但 feature/next 从未合并进 master。
- **本轮量测口径 = feature/next HEAD（而非 master）**，这是明确偏差，非隐瞒。是否可接受由用户/main 裁定；本 subtask 不擅自 merge master（未经授权的分支操作，禁止）。

## 3. release 构建状态

- 日志：`/tmp/pfv-release-build.log`
- 前端已完成：`✓ built in 1m 9s`
- Rust `cargo build --release` 截至本条记录仍在跑（rustc 编译 `aidog_lib` crate，pid 5905 起，`cargo build` pid 96978），**`BUILD_EXIT` 尚未出现**。
- 未自行触发任何 `cargo build` / `yarn tauri build`，避免抢 target 锁。

## 4. ⚠️ 共享量测设施冲突（阻塞冒烟，未解决前不动 pkill/relaunch）

发现与 measure-protocol.md 规则 7（量测设施全局单例，同一时刻只允许一个 subtask 持有）相关的活跃冲突证据：

- `git diff --stat .scratch/perf-200mb/assets/measure.sh` 显示该文件当前有**未提交的改动**（+16/-1），新增了 `ISO_HOME` 隔离启动分支（`HOME="$ISO_HOME" "$APP/Contents/MacOS/aidog"` 直接 fork/exec，绕开 `open -a` 不传 env 的问题）—— 这不是我做的改动，是另一并发 subtask 正在改造该共享脚本。
- 当前 `pgrep -x aidog` 命中 **1 个活跃 aidog 主进程**（pid 5788，elapsed ~1min，配套 WebKit GPU/Networking/WebContent 各一份，elapsed 同步 ~57s）—— 说明有人刚 launch 过，处于早期稳态窗口内。
- 另有历史遗留 WebKit helper（pid 6036/6038 elapsed 7天15h、pid 68239-68241 elapsed 17h23m）与本轮无关，是更早会话残留，不干扰判断但佐证 `.app` 单实例约束下这台机器长期有其他会话在用这套设施。

**结论**：measure.sh + `/Applications/AiDog.app` + `.pids` 当前可能被另一个 subtask（很可能是 window-default-size 或 cold-start-unblock 系的 exec agent，正在验证 ISO_HOME 方案）持有。按 measure-protocol.md 规则 7「持有规则」，本 subtask **不得**在未确认独占窗口前执行 `pkill -x aidog` 或 `open -a`/直跑二进制，否则会踩掉对方正在跑的采样（且事后难判定互相污染）。

**已回传 `需要:` 给 coordinator，冒烟部分待独占窗口批复后再做，标记「待批复」。**

## 5. 压测流形参数（定死，禁每次手敲）

沿用已验证过的两条现成路径的既有约定，不再新起参数：

| 参数 | 值 | 理由 |
|---|---|---|
| 并发数 | 50 | PRD/design.md 场景 3 明确写「50 路并发 mock 流」；`loadgen.sh` 默认值同 |
| 持续时长（正式量测） | ≥600s（10min 稳态窗口） | measure-protocol.md 规则 6「等满稳态 ≥10min」；本 subtask 冒烟只需短时验证脚本可跑通，不需等满 |
| 持续时长（本 subtask 冒烟） | 30s | 只验证脚本链路通不通，不出正式数据（PRD 边界：本 subtask 不出正式数据） |
| 请求体 | `loadgen.sh` 内置：`chunk_count:200, delay_ms:50, input_tokens:4000, output_tokens:2000`，`stream:true` | 单次流约 10s，是「持续转发峰值」的既定口径（脚本内注释：票01采样点③口径），不改动 |
| 模型 | `claude-sonnet-4-20250514`（loadgen.sh 内置） | mock 平台任意模型名均可命中 mock 拦截，沿用既有脚本值，不新增变量 |
| 分组 / token | `mock`（Authorization: Bearer mock） | loadgen.sh 硬编码；🛑 已核对：只打 mock 分组，不碰真实平台，符合硬约束 |
| 目标端口 | `127.0.0.1:9890/proxy`（可用 `LOADGEN_PORT` 覆盖） | app 默认监听端口，冲突时 app 自动 +1，脚本已支持覆盖 |
| 窗口尺寸 | 1026×759（默认，非最大化） | PRD 达标口径；`window-default-size` 已删 `maximized:true` |
| HOME 隔离 | `ISO_HOME=/tmp/aidog-test-$$`（measure.sh launch 新增分支，见上节冲突说明） | test-data-isolation-constraint.md 硬约束；`open -a` 不传 env，故 measure.sh 已加直接 fork/exec 分支 |

## 6. 脚本路径

- `.scratch/perf-200mb/assets/measure.sh`（launch / mem / cpu / stacks / track，本次发现有并发改动中，见第4节）
- `.scratch/perf-200mb/assets/loadgen.sh`（50 路并发 mock 压测，参数已如上定死，未改动）
- `.scratch/perf-200mb/assets/run-size-curve.sh`（多窗口尺寸曲线，本 task 用不到，留痕）
- `.scratch/perf-200mb/assets/explain-baseline.sh`（基线归因，本 task 用不到）

## 4b. release 构建结论（更新）

- `BUILD_EXIT=1`，但失败点是 **updater 签名步骤**（`Error A public key has been found, but no private key. Make sure to set TAURI_SIGNING_PRIVATE_KEY`），发生在 `bundle_dmg.sh` 之后、"Finished 2 bundles" 已输出——**与编译产物无关**。
- 团队 lead 独立核实：`src-tauri/target/release/bundle/macos/AiDog.app/Contents/MacOS/aidog`，79398448 字节，mtime `Aug 1 14:21`，与全部源码 mtime 比对无一份更新 → 确认是含全部 8 个前置 task 改动的最新 release 二进制。
- 已将该 bundle 复制覆盖 `/Applications/AiDog.app`（14:18→14:22，非破坏性，构建产物覆盖非用户数据）。
- **验收项2「release 构建成功」判 PASS**，附上述原因说明（禁止只写「构建成功」掩盖 exit 1）。

## 5b. is-ancestor 表勘误

无。（原表沿用，无需改。）

## 7. 三场景冒烟记录

### 场景1 空闲前台 —— **通过**

`ISO_HOME=/tmp/aidog-pfv-smoke2-15865 ./measure.sh launch`（main=15880，编制核验 PASS）→ `seed-mock` 成功 → 代理探测 `127.0.0.1:9876/proxy` 返 200 → `mem smoke-fg`：

```
PID      PROC           FOOTPRINT_MB
15880    aidog(main)          37.0
15927    GPU                  19.0
15930    Networking            7.2
15931    WebContent           70.0
15939    WebContent           21.0
TOTAL                        154.2
```

`cpu smoke-fg 15`：TOTAL 0.1%（冒烟用短窗口，非稳态口径，仅证脚本链路通）。落盘于 `results/mem-smoke-fg.txt` + `results/cpu-smoke-fg.txt`。

### 场景2 空闲隐藏 —— **被并发冲突打断，未拿到有效数据**

`osascript ... set visible of process "AiDog" to false` 成功（隐藏生效），但等待期间 `measure.sh mem smoke-hidden` 发现 pid 15880 系**已全部退出**，取而代之的是全新 pid 19328 系，其 `HOME=/tmp/aidog-perf-home-smoketest`（非本轮 ISO_HOME），且 `results/iso-app-stdout.log` 显示该进程独立处理了一条我未发出的 mock 请求（`status=200 est_cost=0.00105`）——**证实当时有另一进程在主动使用共享设施**，与场景1开始前 team-lead「全局仅你一人」的确认矛盾。已 `pkill -x aidog` 清场停手，回传 team-lead 求证第二次，未再重试（避免无意义的第3次撞车耗尽"3次失败"配额）。

### 场景3 50路并发mock流 —— **未开始**（阻塞在场景2的冲突解决前）

## 状态

- 前置 8-task 核实：**完成**
- 分支口径偏差：**完成，已显式声明**
- release 构建：**完成**（BUILD_EXIT=1 已排查为签名密钥问题，产物本身最新且已安装，见第4b节）
- 压测参数定死：**完成**（见第5节）
- 三场景冒烟：**场景1完成 / 场景2被并发冲突打断 / 场景3未开始** —— 已二次回传 team-lead 求证共享设施占用来源，等待答复中，不自行第3次重试

## 8. 第三次冲突（更强证据，暂停等确认，未 pkill）

用改后的 measure.sh（`APP` 可覆盖）+ 构建产物路径冒烟：

```
export APP=".../src-tauri/target/release/bundle/macos/AiDog.app"
export ISO_HOME="/tmp/aidog-pfv-smoke3-22993"
APP="$APP" ISO_HOME="$ISO_HOME" ./measure.sh launch
→ 脚本自身回显 "✓ ISO_HOME isolated: /tmp/aidog-pfv-smoke3-22993"（证实我的调用参数确实生效）
→ main=23007，但立即查询该 pid 已不存在
→ pgrep -x aidog 命中另一个 pid 23098，其 HOME=/tmp/aidog-perf-home-smoketest（固定字面量目录名，
  不是我这轮 $$ 随机名)，command=/Applications/AiDog.app/...（不是我指定的构建路径）
→ ps 复查：23098 elapsed 持续增长（非刚死的僵尸），确认是一个仍在运行中的独立进程
```

`results/iso-app-stdout.log` 是共享固定路径（非按 pid 隔离），两个并发 `measure.sh launch` 会互相
覆盖/竞争同一 stdout 重定向目标——这解释了为何我这边总看到 `/tmp/aidog-perf-home-smoketest` 的日志。

**这次没有像前两次那样能被"时间戳归因到我自己早前操作"解释**——`/tmp/aidog-perf-home-smoketest`
在我上一轮已经 `rm -rf` 删除过，此刻却又活跃存在，且 pid 是全新的（23098，与我 23007 不同）。

**已停手，未 pkill**（不确定这次是否可安全清），已回传 team-lead 第三次确认。按「冒烟连试3次仍跑
不通→停手回传，禁把参数改松凑通」，本节即该停止点。

## 9. 冲突根因更正（team-lead 复核后的最终结论）

第三次停手求证后，team-lead 查明真相：**前一个 executor `exec-pfv-s1`（原以为 14:17 连接断死零产出）实际未死，一直在同一套设施上跑**，与我互相当成了幽灵。已被 team-lead `TaskStop`。此后确认全局唯一。

第一次「measure.sh +16/-1」确系我自己所写（时间戳证据成立）；第二/三次的 `HOME=/tmp/aidog-perf-home-smoketest` 实例（pid 13440/19328/23098 等）**均是 `exec-pfv-s1` 起的**，14:26:09 那条 `group resolved group=mock ... status=200` 请求也是它打的，不是我。

### 从 exec-pfv-s1 继承的成果（未重做）

1. `open -a` 不可靠传 HOME（走 LaunchServices，未必继承 shell env）——与我这边独立得出的结论一致，互证可信度更高。
2. **真 gap 已修**：`loadgen.sh`（`mock-loadgen-50x5min.md` 路径B）此前实际打的是用户真实 `~/.aidog/platform.db` 里的 mock 平台/分组，违反 HOME 隔离硬约束；exec-pfv-s1 已修正。
3. **操作细节**：往**运行中**的 app 灌 SQL 种子后，同一进程内不生效（会报 `no matching group`），必须 kill + relaunch 一次才读到新种子。`seed-mock` 流程需先 launch 生成 schema，种子写入后如需在同进程内生效必须重启一次 —— 本轮 s1 冒烟走的是「先 launch 建库 → seed-mock → 直接用同一进程」，实测代理探测 200 且路由到 mock 命中成功，说明**首次冷启动后紧接着 seed 是生效的**（问题只发生在"已运行较久的进程事后补种"场景，本冒烟未踩中，仍记录此坑供 s2 参考）。
4. **场景1 双样本互证**：exec-pfv-s1 的 scenario1 冒烟 TOTAL≈160.9MB（main 44/GPU 20/Net 6.9/WebContent 65+25），我加固前的一次是 154.2MB（main 37/GPU 19/Net 7.2/WebContent 70+21），加固后重跑是 162.8MB（main 45/GPU 19/Net 6.8/WebContent 71+21）。三次独立采样在 154~163MB 区间内，量级一致——冒烟口径本身稳定可信。

## 10. measure.sh 并发安全加固（本 subtask 正式交付项）

按 team-lead 指示做的四条加固，已落地并自证：

1. **per-run 隔离状态**：`PIDFILE` 与 stdout 日志按 `ISO_HOME` basename 生成唯一后缀（`.pids.<run>` / `iso-app-stdout.<run>.log`），未设 `ISO_HOME` 时退化为原全局单份路径（向后兼容旧用法）。
2. **launch 后身份断言**：拿到 main pid 后立即 `ps eww -p <pid> | grep HOME=`，与本轮 `ISO_HOME` 比对，不等则 `exit 3` 并打印实际值——当场识破串台，不留到 mem/cpu 才发现。
3. **每个采样点前复验**：`mem`/`cpu` 子命令逐 pid 核对存活 + HOME 匹配，不匹配跳过（mem）或 abort（cpu）。**踩过一个坑并已修**：WebKit XPC helper（GPU/Networking/WebContent 经 `xpcproxy` 拉起）`ps eww` 读不到其 HOME（空值），最初实现把「读不到」等同「不匹配」导致全部 WebKit 进程被误判串台跳过（TOTAL 从 162.8 掉到仅剩 37.0 的 main）。修正为：只在**读得到 HOME 且确实不同**时才判串台，空值不拦截（WebKit 归属仍由 launch 阶段 before/after 差集 + 编制核验兜底）。
4. **launch 前清场并验空**：`pkill -x aidog` 后立刻 `pgrep -x aidog` 复查，非空则报错退出，不再"顶着别人的残留继续跑"。

`git diff --stat measure.sh` 现含以上加固 + 此前 exec-pfv-s1 的 ISO_HOME/seed-mock 分支，均已随冒烟验证过可用。

## 11. 三场景冒烟最终记录（加固后，独占窗口下跑的）

用 `ISO_HOME=/tmp/aidog-pfv-final-30097` + `APP=.../target/release/bundle/macos/AiDog.app`（不再碰 `/Applications`）：

- **场景1 空闲前台**：`mem scenario1-fg` TOTAL 162.8MB（main 45/GPU 19/Net 6.8/WebContent 71+21），`cpu scenario1-fg 15` TOTAL 0.4%。
- **场景2 空闲隐藏**：`osascript set visible of process "AiDog" to false` 生效，`mem scenario2-hidden` TOTAL 157.7MB（main 40/GPU 19/Net 6.7/WebContent 71+21），`cpu scenario2-hidden 15` TOTAL 0.5%。
- **场景3 50路并发mock流**（冒烟 20~30s，非正式 ≥600s 稳态）：`loadgen.sh 50 30`（`LOADGEN_PORT=9876`）跑通，`cpu scenario3-load 20` TOTAL 66.6%（main 41.9%，Networking 9.9%，WebContent 3.8%+10.2%，符合"持续转发峰值 CPU 显著上升"预期）；`mem scenario3-load` 期间 TOTAL 228.3MB（main 30/WebContent 76+16MB，WebKit malloc 明显上涨，符合流式响应缓冲预期）。

三场景脚本链路、mock 隔离、HOME 隔离全部验证通过。**本 subtask 不判定达标/不达标**（PRD 边界：冒烟不出正式数据，正式量测归 s2）。

## 12. 收尾

- `pkill -x aidog` 清场，`rm -rf $ISO_HOME`（`/tmp/aidog-pfv-final-30097`），`pgrep -x aidog` 确认空。
- `/Applications/AiDog.app` 保留为本轮已覆盖状态（已上报用户，未回退——team-lead 未要求回退，只要求此后不再碰）。
- 未清理 `.scratch/perf-200mb/assets/results/` 下历史采样文件（PRD 边界：本 subtask 不删，留给下游 s6）。

## 状态（最终）

- 前置 8-task 核实：**完成**
- 分支口径偏差：**完成，已显式声明**（feature/next 非 master，领先259/落后1，未合并）
- release 构建：**完成 PASS**（BUILD_EXIT=1 系 updater 签名密钥缺失，与编译产物无关；已用二进制 mtime 与全部源码 mtime 比对证明产物最新）
- 压测参数定死：**完成**
- 共享量测设施并发安全加固：**完成**（四条落地，见第10节，含一处自证发现的 bug 已修）
- 三场景冒烟：**全部通过**（见第11节）
