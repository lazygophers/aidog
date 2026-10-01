#!/usr/bin/env python3
"""解析 /usr/bin/sample 文本的 Call graph：去掉阻塞等待叶子，出 on-CPU 自身耗时 Top N 与各模式的包含占比。
用法: sample_analyze.py <sample.txt> [topN]   （符号经 rustfilt 反混淆）
"""
import re, subprocess, sys
from collections import Counter

WAIT = {"semaphore_wait_trap", "__psynch_cvwait", "__workq_kernreturn", "kevent", "swtch_pri",
        "__psynch_mutexwait", "__ulock_wait", "__ulock_wait2", "mach_msg2_trap", "__semwait_signal"}
IO = {"pwrite", "fsync", "pread", "ftruncate", "writev", "write", "__recvfrom", "read", "__sendto", "__fcntl"}
# 审计嫌疑 → 反混淆后路径里的子串（任一命中即计入；互不排斥）
SUSPECTS = [
    ("1 proxy_log 大字段写库（aidog_logs + SQLite 执行）", r"aidog_logs::proxy_log|update_proxy_log_columns|insert_proxy_log_columns|large_fields|from_log"),
    ("1' 全部 SQLite 执行（含 WAL/fsync）", r"sqlite3|walChecksum|tokio_rusqlite"),
    ("3 请求体深复制/序列化/pretty", r"format_pretty_json|PrettyFormatter|converter::request|forward_attempt.*to_string|serde_json::ser"),
    ("3' serde_json 反序列化（全部）", r"serde_json::de|serde_json::value::de|serde_json::read"),
    ("3'' serde_json::Value clone/drop", r"Value as core::clone::Clone|drop_in_place<serde_json::value::Value>|clone_subtree"),
    ("5 流式逐帧解析", r"feed_sse_usage|parse_upstream_sse|has_stream_terminator|proxy::stream|proxy::finish"),
    ("6 SQLite 往返/模型条目/trace", r"model_entry|get_platform|aidog_db::trace|sql_profile_callback"),
    ("7 tracing/日志格式化", r"tracing_subscriber|tracing_appender|aidog_db::logging"),
    ("regex（全部）", r"regex_automata|regex::|aho_corasick"),
    ("网络 IO（hyper/reqwest/tokio io）", r"hyper::|reqwest::|h2::|tokio::net|mio::"),
    ("malloc/free/memmove（叶子）", r"^(_xzm|_malloc|_free|_platform_mem|DYLD-STUB|<dedup)"),
]
# 追加嫌疑：env SUSPECTS_EXTRA="标签=正则;;标签=正则"（票 06 用来拆流式逐帧各函数）
import os
SUSPECTS += [tuple(x.split("=", 1)) for x in os.environ.get("SUSPECTS_EXTRA", "").split(";;") if "=" in x]
LINE = re.compile(r"^(?P<pre>[ +!:|]*)(?P<n>\d+) (?P<name>.+?)(?:  \(in [^)]*\).*)?$")

def demangle(names):
    out = subprocess.run(["rustfilt"], input="\n".join(names), capture_output=True, text=True).stdout.split("\n")
    return dict(zip(names, out))

def main():
    path = sys.argv[1]; top = int(sys.argv[2]) if len(sys.argv) > 2 else 15
    lines = open(path, errors="replace").read().split("\n")
    s = lines.index("Call graph:") + 1
    nodes = []  # (depth, count, name)
    for l in lines[s:]:
        if not l.strip(): break
        m = LINE.match(l)
        if not m: continue
        nodes.append((len(m["pre"]), int(m["n"]), m["name"].strip()))
    # 叶子自身样本 = count - 子节点 count 之和；同时记下根到叶路径
    samples = []  # (path_names, self_count)
    stack = []
    for i, (d, n, name) in enumerate(nodes):
        while stack and stack[-1][0] >= d: stack.pop()
        stack.append((d, n, name))
        child = 0
        # 直接子节点求和
        j = i + 1; cd = None
        while j < len(nodes) and nodes[j][0] > d:
            if cd is None: cd = nodes[j][0]
            if nodes[j][0] == cd: child += nodes[j][1]
            j += 1
        self_n = n - child
        if self_n > 0: samples.append(([x[2] for x in stack], self_n))
    dm = demangle(sorted({nm for p, _ in samples for nm in p}))
    total = sum(c for _, c in samples)
    wait = sum(c for p, c in samples if p[-1] in WAIT)
    io = sum(c for p, c in samples if p[-1] in IO)
    cpu = total - wait - io
    print(f"samples total={total} wait={wait} io_syscall={io} on_cpu={cpu}")
    selfc = Counter()
    for p, c in samples:
        if p[-1] in WAIT or p[-1] in IO: continue
        selfc[dm.get(p[-1], p[-1])] += c
    print(f"\n## Top {top} self (on-CPU, 不含等待/IO 系统调用)")
    for name, c in selfc.most_common(top):
        print(f"{c:6d} {100*c/cpu:5.1f}%  {name[:160]}")
    print("\n## 嫌疑包含占比（分母 on_cpu；io 列=该路径下 IO 系统调用样本）")
    for label, pat in SUSPECTS:
        r = re.compile(pat)
        c = sum(n for p, n in samples if p[-1] not in WAIT and p[-1] not in IO and any(r.search(dm.get(x, x)) for x in p))
        ci = sum(n for p, n in samples if p[-1] in IO and any(r.search(dm.get(x, x)) for x in p))
        print(f"{c:6d} {100*c/cpu:5.1f}%  io={ci:5d}  {label}")

main()
