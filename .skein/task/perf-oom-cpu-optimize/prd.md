# perf-oom-cpu-optimize — 高 CPU / OOM 性能优化

## 目标
- [x] 深度解析「持续高 CPU + OOM」根因(原触发场景未明)
- [x] 实施修复止血(OOM 内存有界)+ 优化(CPU 热路径砍除)

## 用户价值
- OOM 不再发生: 流式内存有界(≤16MB/请求) + 默认不累积上游 body
- 持续高 CPU 消除: tray-refresh 风暴 + get_platform 重复查询砍除

## 边界
- [x] 仅改 `gateway/proxy/` + `db/proxy_log.rs` + `src-tauri/src/app_setup.rs`(tray debounce)
- [x] 不改公开 API / Tauri command 签名 / serde 字段名 / DB schema(向后兼容)
- [x] 不加新依赖
- [x] 前端无需改(emit 节流后前端 listener debounce 不变)

## 非目标
- 不重构 proxy 模块架构
- 不改 retention 清理逻辑(独立维度)
- 不加运行时 profile 基线(用户无数据, 按静态审计直接修高置信项)

## 验收标准
- [ ] `cargo clippy --all-targets` 0 新 warning(baseline 115)
- [ ] `cargo test --workspace` 通过(除 master baseline 9 个 test_headers)
- [ ] `STREAM_BODY_MAX_BYTES ≤ 16MB`
- [ ] `record_upstream_body` 默认 false
- [ ] snapshot(ProxyLogColumns) 不持 body 字段
- [ ] `upsert_log` 仅终态 emit `proxy-log-updated` + `tray-refresh`
- [ ] tray-refresh debounce ≥ 200ms
- [ ] platform/price 请求级算一次
- [ ] proxy_log 写入/读取不破坏

## 索引
- 详细设计: [design.md](design.md)
- 审计发现: [findings.md](findings.md)
- 调度: task.json(`skein.py subtask list perf-oom-cpu-optimize`, 3 subtask 串行)
