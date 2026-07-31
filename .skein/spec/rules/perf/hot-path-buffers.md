---
title: hot-path-buffers
category: perf
keywords: [mpsc,capacity,try_send,背压,深拷贝,热路径,TOCTOU]
status: active
inclusion: auto
anchors: src-tauri/crates/aidog_core/src/gateway/proxy/log.rs
---

## mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝

mpsc 队列热路径丢弃分支：先 `Sender::capacity() == 0` 判队满再 return，避免为「确定要被丢弃」的消息
（try_send 会因 Full 返回错误）付出昂贵深拷贝构造成本。适用场景：try_send 非阻塞丢弃型背压 + 消息体含
大 String/Vec 等重克隆字段。TOCTOU 权衡：check-then-send 存在极小竞态窗口（多 producer 场景 capacity
检查后被并发填满），可接受——退化为回到原 try_send 路径正常处理 Full/Closed，不引入正确性问题，只是
偶发未省下这次克隆。closed channel 场景不特判（罕见 shutdown 窗口，走原有 match 分支即可）。
