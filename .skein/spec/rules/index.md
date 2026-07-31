# SKEIN rules 规则索引 (章节粒度: 一行一条规则)

类目: perf(1) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | inclusion | anchors | status/出链 | summary |
|---|---|---|---|---|---|---|---|
| perf/hot-path-buffers.md#mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝 | perf | mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝 | mpsc,capacity,try_send,背压,深拷贝,热路径,TOCTOU | auto | src-tauri/crates/aidog_core/src/gateway/proxy/log.rs | active | mpsc 队列热路径丢弃分支：先 `Sender::capacity() == 0` 判队满再 return，避免为「确… |
