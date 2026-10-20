#!/usr/bin/env python3
"""Aidog request inspector — 经 aidog-kernel 管理面 (/rpc) 读取一条 proxy_log 并格式化输出。

背景: SQLCipher 全库加密落地后(spec db-sqlcipher-full 票 01, 2026-10-20), sqlite3 直查
~/.aidog/log.db 报 "file is not a database", 本脚本改走内核命令
POST /rpc/proxy_log_get 与 /rpc/proxy_log_list_filtered。

用法:
  python3 req_inspect.py <request_id>              # 查单条(摘要 + 各 body, body 默认截断)
  python3 req_inspect.py <request_id> --full       # 不截断 body
  python3 req_inspect.py <request_id> --raw        # 输出原始 JSON(机读, 不脱敏格式化)
  python3 req_inspect.py --recent [N]              # 最近 N 条摘要(默认 10)
  python3 req_inspect.py --recent 20 --group glm-coding-plan-auto   # 过滤 group
  python3 req_inspect.py --recent 20 --status 400  # 过滤状态码 (200=仅成功, -1=仅失败, 其它=精确码)
  python3 req_inspect.py --port 9891 --token <t>   # 管理面端口/凭据 (默认 127.0.0.1:9891, 无凭据)

前置: 需有 aidog-kernel --ui 在跑(读写同一 ~/.aidog 数据 + keychain 密钥)。
  aidog-kernel --ui --port 9891
约定:
  - headers 里的鉴权字段(authorization/x-api-key/x-goog-api-key/api-key/cookie)自动脱敏。
  - body 若是 JSON 则 pretty-print; 默认截断 4000 字符, --full 关闭截断。
"""
import sys, json, argparse, urllib.request, urllib.error

DEFAULT_PORT = 9891
TRUNC = 4000
SENSITIVE_KEYS = {"authorization", "x-api-key", "x-goog-api-key", "api-key",
                  "apikey", "cookie", "set-cookie", "x-goog-api-client"}

# 无代理 opener: 本机 127.0.0.1 不走环境变量里的 HTTP_PROXY
_OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def rpc(base, token, name, args):
    req = urllib.request.Request(
        f"{base}/rpc/{name}", method="POST",
        data=json.dumps(args).encode(),
        headers={"content-type": "application/json"})
    if token:
        req.add_header("authorization", f"Bearer {token}")
    try:
        with _OPENER.open(req, timeout=10) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        body = e.read().decode(errors="replace")
        sys.exit(f"/rpc/{name} HTTP {e.code}: {body}")
    except urllib.error.URLError as e:
        sys.exit(f"连不上内核管理面 {base}: {e.reason}\n"
                 f"先起内核: aidog-kernel --ui --port <端口>  (纯内核形态无 /rpc)")


def mask(val):
    """脱敏: 字符串保留头尾少量, 中间打码。"""
    if not isinstance(val, str) or len(val) <= 8:
        return "****"
    return val[:4] + "****" + val[-4:]


def desensitize_headers(raw):
    """headers 是 JSON 字符串; 解析后脱敏鉴权字段, 返回 pretty JSON 字符串。"""
    try:
        h = json.loads(raw) if raw else {}
    except Exception:
        return raw  # 非 JSON 原样返回
    if isinstance(h, dict):
        for k in list(h.keys()):
            if k.lower() in SENSITIVE_KEYS:
                v = h[k]
                h[k] = mask(v if isinstance(v, str) else json.dumps(v))
    return json.dumps(h, ensure_ascii=False, indent=2)


def fmt_body(raw, full):
    """body: 尝试 pretty JSON, 否则原文; 按需截断。"""
    if raw is None or raw == "":
        return "(空)"
    s = raw
    try:
        s = json.dumps(json.loads(raw), ensure_ascii=False, indent=2)
    except Exception:
        pass
    if not full and len(s) > TRUNC:
        return s[:TRUNC] + f"\n... [截断, 共 {len(s)} 字符, 用 --full 看全部]"
    return s


def fmt_ts(epoch):
    try:
        import datetime
        # epoch 可能是秒或毫秒
        e = int(epoch)
        if e > 10_000_000_000:
            e //= 1000
        return datetime.datetime.fromtimestamp(e).strftime("%Y-%m-%d %H:%M:%S")
    except Exception:
        return str(epoch)


def print_detail(d, full, raw_mode):
    if raw_mode:
        print(json.dumps(d, ensure_ascii=False, indent=2))
        return

    ok = "✓" if d["status_code"] == 200 else "✗"
    print(f"=== Request {d['id']} {ok} ===")
    print(f"时间      : {fmt_ts(d['created_at'])}")
    print(f"group     : {d['group_key']}")
    print(f"model     : {d['model']}  ->  actual: {d['actual_model'] or '(未路由)'}")
    print(f"protocol  : {d['source_protocol'] or '?'}  ->  {d['target_protocol'] or '?'}")
    print(f"platform  : id={d['platform_id']}")
    print(f"status    : client={d['status_code']}  upstream={d['upstream_status_code']}")
    print(f"耗时      : {d['duration_ms']} ms   stream={'是' if d['is_stream'] else '否'}   retry={d['retry_count']}")
    print(f"tokens    : in={d['input_tokens']} out={d['output_tokens']} cache={d['cache_tokens']}   est_cost=${d['est_cost']:.6f}")
    print(f"client URL: {d['request_url']}")
    print(f"upstream  : {d['upstream_request_url']}")

    # 重试链 (kernel 已把 attempts 列反序列化成数组对象)
    att = d.get("attempts") or []
    if att:
        print("\n--- 重试链 (attempts) ---")
        for i, a in enumerate(att):
            err = a.get("error") or "-"
            print(f"  [{i}] platform={a.get('platform_name')}(id={a.get('platform_id')}) "
                  f"status={a.get('status_code')} {a.get('duration_ms')}ms err={err}")

    print("\n--- 入站请求 headers (脱敏) ---")
    print(desensitize_headers(d["request_headers"]))
    print("\n--- 入站请求 body ---")
    print(fmt_body(d["request_body"], full))
    print("\n--- 上游请求 headers (脱敏) ---")
    print(desensitize_headers(d["upstream_request_headers"]))
    print("\n--- 上游请求 body ---")
    print(fmt_body(d["upstream_request_body"], full))
    print("\n--- 上游响应 headers ---")
    print(desensitize_headers(d["upstream_response_headers"]))
    print("\n--- 响应 body (回客户端) ---")
    print(fmt_body(d["user_response_body"] or d["response_body"], full))


def print_recent(base, token, n, group, status):
    flt = {}
    if group:
        flt["group_key"] = group
    if status is not None:
        flt["status"] = status
    page = rpc(base, token, "proxy_log_list_filtered", {"filter": flt, "limit": n, "offset": 0})
    rows = page.get("items") or []
    if not rows:
        print("无匹配记录")
        return
    print(f"最近 {len(rows)} 条:")
    print(f"{'id':32}  {'time':19}  {'status':6}  {'ms':>6}  {'out':>5}  group / model")
    for r in rows:
        ok = "200" if r["status_code"] == 200 else f"!{r['status_code']}"
        print(f"{r['id']:32}  {fmt_ts(r['created_at']):19}  {ok:6}  {r['duration_ms']:>6}  "
              f"{r['output_tokens']:>5}  {r['group_key']} / {r['actual_model'] or r['model']}")
    print("\n用 python3 req_inspect.py <id> 看单条详情")


def main():
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("request_id", nargs="?")
    ap.add_argument("--port", type=int, default=DEFAULT_PORT)
    ap.add_argument("--token", default="")
    ap.add_argument("--full", action="store_true")
    ap.add_argument("--raw", action="store_true")
    ap.add_argument("--recent", nargs="?", const=10, type=int)
    ap.add_argument("--group")
    ap.add_argument("--status", type=int)
    a = ap.parse_args()

    base = f"http://127.0.0.1:{a.port}"
    if a.recent is not None:
        print_recent(base, a.token, a.recent, a.group, a.status)
        return
    if not a.request_id:
        sys.exit("需要 <request_id>, 或用 --recent [N] 列最近请求")
    d = rpc(base, a.token, "proxy_log_get", {"id": a.request_id})
    if d is None:
        sys.exit(f"未找到 request id: {a.request_id} (确认 id 正确且未被清理)")
    print_detail(d, a.full, a.raw)


if __name__ == "__main__":
    main()
