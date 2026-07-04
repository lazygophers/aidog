# PRD — CONNECT 隧道 read_buf flush 写错对象致 TLS 握手 RST

## 背景
`curl -v -x http://127.0.0.1:9892/proxy https://www.baidu.com`：CONNECT 200 返回 → curl 发 TLS ClientHello → `Connection reset by peer`。

## 根因（bug-hunt 诊断证据闭环）
hyper-util auto h1 server 写完 CONNECT 200 响应后 **speculative read** 客户端下一请求，把客户端随后发的 TLS ClientHello 预读进 `parts.read_buf`。代码应把这些「从客户端读到的字节」flush 到 **upstream**，却写成 `write_all(&mut client, &parts.read_buf)` 回灌客户端：
- 上游永远收不到 ClientHello → 上游 TLS 无响应
- 客户端收到自己字节的乱序回灌 → TLS 状态机错乱 → RST

证据：
- `connect.rs:292`（spawn_blind_relay P1 路径，baidu 走这条）+ `connect.rs:226`（MITM 降级路径，复制粘贴同 bug）
- `connect.rs:306` 注释明写「`prefetch` 须先 flush 到上游」与实现冲突
- git blame `024a04c4`（P1 初版）即笔误，`9ccbfda9` 重构 bridge_bidir 保留
- 现有测试 `connect_tunnel_via_real_proxy_env`（test_connect.rs:137）用裸 TcpStream 严格串行（write CONNECT → 读响应 → 再 write），speculative read 窗口已过 read_buf 空，不触发；真实 TLS 客户端 200 后立即流水线 ClientHello 落进预读缓冲 → 触发

## 目标
D1 修 spawn_blind_relay 路径 flush 方向 + D2 修 MITM 降级路径（接上已有 `_prefetch` 参数）+ D3 删过时注释 + D4 新增回归测试暴露原 bug

## 产出

### D1 — spawn_blind_relay 路径（connect.rs:290-294）
read_buf 是从客户端读的字节，flush 到 upstream（非 client）：
```rust
let client = TokioIo::new(parts.io);
let mut upstream = upstream;   // 原 move 进 bridge_bidir 的 TcpStream
if !parts.read_buf.is_empty() {
    let _ = tokio::io::AsyncWriteExt::write_all(&mut upstream, &parts.read_buf).await;
}
bridge_bidir(client, upstream).await;
```

### D2 — MITM 降级路径（connect.rs:223-231 + blind_relay_after_connect L318-343）
upstream 在 `blind_relay_after_connect` 内部才 connect，flush 须在 connect 之后。该函数已有 `_prefetch: &[u8]` 参数（L328）专为此用，去 `_` 接实参：
- 调用侧 L228-231：`&parts.read_buf` 作 prefetch 实参；删 L225-227 的 `write_all(&mut client)`
- 函数签名 `_prefetch` → `prefetch`；Ok 分支 `upstream` 加 `mut`，connect 成功后 `write_all(&mut upstream, prefetch)` 再 bridge_bidir

### D3 — 删/改过时注释
connect.rs:200-206「合法 CONNECT 客户端 read_buf 必空」论断被证伪，改为「read_buf 非空时 blind_relay 已正确 flush 到 upstream」。L224-226、L239 同步。

### D4 — 回归测试（暴露原 bug）
仿 `connect_tunnel_via_real_proxy_env`（test_connect.rs:137），客户端写完 CONNECT **不等响应**立即在同一 socket 连续写 CONNECT + payload（模拟 hyper-util speculative read 命中）。断言上游收到 payload 首字节（或 echo 回正确 payload，非回灌乱序）。当前 bug 会失败，修复后通过。

## 验证
- [ ] `cd src-tauri && cargo test gateway::proxy::test_connect`（13 既有 + 1 新增全过）
- [ ] `cargo clippy` 0 warning
- [ ] `curl -v -x http://127.0.0.1:9892/proxy https://www.baidu.com` 实测完成 TLS 握手 + 200（subagent 实测或 main 指引）
- [ ] 改动仅在 connect.rs + test_connect.rs（单 worktree）

## 非目标
- ❌ 改 MITM accept_client / serve_plaintext 路径（read_buf 非空已降级 blind_relay，D2 修复后端到端正确）
- ❌ 改白名单 / 路由 / 响应头（诊断已排除）
- ❌ 重构 bridge_bidir

## grill 自审 trace
- 轴 A 目标 ✓ 修 flush 方向 + 测试，封闭
- 轴 B 产出 ✓ connect.rs 两处 + 注释 + 测试，可验收
- 轴 C 验证 ✓ cargo test + clippy + curl 实测可执行
- 轴 D 资源 ✓ connect.rs + test_connect.rs 单文件对
- 轴 E 依赖 ✓ 单文件无并行冲突，单 subtask 一次做
- 轴 F 失败 ✓ 根因证据闭环（git blame + 注释自相矛盾），修复方向确定
- 轴 G 检查点 ✓ 根因经 bug-hunt 证据闭环，无需用户决策
