# PRD — MITM CA macOS osascript 安装失败修复

## 现象（用户 2026-07-03 实机，两问题）
**问题 1 — 自动装失败无错误提示**
- macOS 点「安装 CA」→ mitm_install_ca_prepare 写 ca.pem → 前端 Command.create("macos-trust-ca", args).execute() → **osascript 失败**（installed=false）
- **兜底弹窗出现**（manual_display 真实 sudo 命令）但**只给手动命令，不显失败原因 exit/stderr** → 用户感知"无错误提示"
- toast 文案可能被忽略或 i18n 缺失返空（用户报告"没有文案"）
- 日志：mitm_set_ca_installed installed=false（距 prepare 4 秒 = osascript 有交互）

**问题 2 — 手动装成功页面不更新**
- 用户复制兜底弹窗命令到终端手敲 `sudo security add-trusted-cert ...` **成功**（CA 实际进 System.keychain）
- 但 `ca_installed` 只在 shell execute ok 后 `setCaInstalled(true)` 回写 DB；手动装脱离 app 进程 → DB 不更新
- `mitm_status` 读 DB ca_installed 静态字段 → 仍 false → 页面显"已装信任库：否"
- **缺 keychain 实状校验机制**（用户实际已装，app 不感知）

## 根因假设（已排除项 + 待实证）
**已排除**：
- ❌ security 命令本身错（用户手敲 `sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain /Users/luoxin/.aidog/mitm-ca.pem` **成功**）
- ❌ capability scope validator 不匹配（validator regex `do shell script "...System\.keychain /.+\.pem" with administrator privileges` 与 ca.rs 产出 args 字面 fullmatch，理论通过）
- ❌ 路径含空格（aidog_data_dir = ~/.aidog 无空格）

**待实证（最可能）**：
1. osascript `do shell script "..." with administrator privileges` 在 Tauri 应用进程下行为异常（GUI 上下文 / Aqua session / 密码框传递）→ exit 1
2. **toast 无文案根因**：`if(!ok)` 分支 setError(base) 必跑，但用户不见文案 → 怀疑 (a) out.code 结构异常（undefined/null 致 ok=false 但分类文案逻辑异常）(b) out.stderr 为空 + classifyTrustError 落 cmd_fail 但 setError 渲染被吞 (c) catch 分支 String(e) 为空（reject 无 message）
3. capability 实际未生效（mitm-ca.json 结构 / Tauri glob 未加载）→ Command.create reject

## 阶段 A — 诊断实证（先做，必做）
加运行时诊断拿真实根因，禁继续静态猜：
1. **后端 tracing**：mitm_install_ca_prepare 落 ca_pem_path 日志（已有），新增 set_ca_installed 落 installed + 可选 out 元数据
2. **前端诊断**：handleInstallCa 加 `console.error("ca install result", { code: out.code, stderr: out.stderr, stdout: out.stdout, signal: out.signal })`（不论 ok/!ok 都打）；setError 兜底确保非空（`base || String(out)`）
3. **用户复现**：跑一次 → 看浏览器/devtools console + toast 实际文案 + stderr 原文 → 根因确定

## 阶段 B — osascript 根因修（阶段 A 诊断 merge 后用户复现 stderr 定）
阻塞等用户复现 `[ca-install]` console 日志 + 兜底弹窗 exit/stderr 红框。根因定后据实修：
- 若 osascript 进程上下文问题 → 换提权机制（候选：osascript 加 `with prompt` / AppleScript `tell application "Security Agent"` / 直接 sudo via helper / 文档化手敲）
- 若 out 结构异常 → 修 setError 分支 + classifyTrustError 处理 null/undefined code
- 若 capability 未生效 → 修 mitm-ca.json 结构 / 显式注册

## 阶段 C — keychain 实状校验 + status 自动查（问题 2，不依赖阶段 B）
**决策（用户 2026-07-03）**：status() 自动查 keychain 实状（非手动按钮）。

**后端**（ca.rs + commands/mitm.rs）：
1. ca.rs 加 `verify_trust_installed(cert_pem: &str) -> bool` —— std::process::Command 子进程查实状（非 tauri-plugin-shell，无需 capability，后端直跑）：
   - macOS: `security find-certificate -c "AirDog MITM CA" -p /Library/Keychains/System.keychain` exit 0 → true（CN 固定，读公开无需 sudo）
   - Windows: `certutil -store Root` 输出含 "AirDog MITM CA" → true
   - Linux: `test -f /usr/local/share/ca-certificates/aidog-ca.crt` → true
2. ca.rs 加 `sync_ca_installed_from_system(db, root_ca) -> bool` —— 调 verify，与 DB ca_installed 不一致则 set_ca_installed 回写，返实状
3. commands/mitm.rs `mitm_status`：load_root_ca 后若 ca_present，调 sync_ca_installed_from_system，status.ca_installed 取实状（非 DB 静态值）

**前端**：无需改（status() 返实状后页面自动显"已装：是"，refresh 已在）。

## 验收
- [ ] macOS 点「安装 CA」→ osascript admin 弹密码框 → 输密码 → CA 进系统信任库（security find-certificate 验证）
- [ ] 失败时兜底弹窗显 exit/signal/stderr/stdout（阶段 A 诊断）+ toast 必显示有意义的错误文案（非空）
- [ ] 兜底弹窗仍作 fallback（D8 不破）
- [ ] **问题 2**：手动装成功后进/刷新 MITM 页 → status() 自动查 keychain → 页面显"已装信任库：是"（无需手动点校验）
- [ ] **问题 2**：手动卸载后 refresh → 页面显"否"（verify 双向同步）
- [ ] yarn build 绿 + cargo test（ca_ 系列）过 + cargo clippy 0 warn

## 非目标
- 不改 Windows / Linux 提权链路（本 task 仅 macOS 实机 bug）
- 不改 MITM 白名单 / CA 生成逻辑
- 不改 Tauri command 签名

## 调度
- **本轮 subagent（阶段 A merge + 阶段 C 一次做）**：worktree `.worktrees/07-03-mitm-ca-macos-fix` 内，阶段 A 诊断已在（commit 738e1bfa），增量做阶段 C（后端 verify + status 集成）。finish 时一次 merge master 带走 A+C。
- **阶段 B（osascript 根因修）**：阻塞，等 A+C merge 后用户复现 stderr 另起一轮。
- 并发：arch-redesign research 已完（fan-out 位腾出），本 task 与 arch implement 可并行（文件集不相交：mitm 改 ca.rs/commands/mitm.rs/MitmConfig.tsx vs arch 改 pages/5 文件）。
