# 隔离压测环境（真实转发路径，经 forward_attempt）

首次完整跑一遍（约 4 分钟构建 + 10 秒其余）：

```bash
H=<repo>/scripts/perf（本仓库即 /Users/luoxin/persons/lyxamour/aidog/scripts/perf）
$H/build.sh                 # cargo build --release -p aidog_kernel → src-tauri/target/release/aidog-kernel
node $H/gen_bodies.mjs      # → $H/results/bodies/body-{280k,1m}[-nostream].json（确定性，290551 / 1050785 字节）
$H/start.sh                 # 假上游 :18901 + 隔离内核（HOME=/tmp/aidog-perf-default，代理 :19890，管理面 :19891）
$H/seed.sh                  # 经 /rpc 建 2 平台 + 2 分组（已灌过则跳过）
$H/smoke.sh                 # oha -n 1 打两组，打印状态分布、假上游计数、隔离 log.db 最新行
$H/stop.sh                  # 只停 pid 文件里、命令行与 HOME 都核对过的进程
```

## 两条路径

| 分组 group_key（请求头 `x-api-key`） | 平台 | 上游 URL | 走的代码 |
|---|---|---|---|
| `perf-conv` | openai，base_url `http://127.0.0.1:18901/v1` | `/v1/chat/completions` | anthropic→openai 协议转换 |
| `perf-pass` | anthropic，base_url `http://127.0.0.1:18901` | `/v1/messages` | 同协议透传 |

客户端一律打 `http://127.0.0.1:19890/v1/messages`（anthropic 入站）。压测示例：

```bash
oha -z 60s -c 50 --no-tui -m POST -T application/json -H 'x-api-key: perf-conv' \
  -H 'anthropic-version: 2023-06-01' -D $H/results/bodies/body-280k.json http://127.0.0.1:19890/v1/messages
```

## 旋钮（环境变量）

- `PERF_ID=<id>`：隔离目录 `/tmp/aidog-perf-<id>`（默认 `default`）；同时跑两套需再改三个端口。
- `PROXY_PORT` / `MGMT_PORT` / `MOCK_PORT`：默认 19890 / 19891 / 18901；任一为 9890 脚本直接拒绝。
- `UI=0 start.sh`：纯内核（无管理面，`seed.sh` 不可用；先用 UI=1 灌好再 stop → UI=0 start）。
- `CHUNKS` / `DELAY_MS` / `CHUNK_TEXT`：假上游每响应块数 / 块间隔毫秒 / 每块文本（默认 20 / 20 / `"hello "`），改后需 stop+start。
- `LOG_BODIES=0 seed.sh`：只记元数据（默认 1 = 镜像用户实测的 log_user_request/log_upstream_request 全开）。只对新 PERF_ID 生效（已灌过会跳过）。
- `BODY=<file> smoke.sh`：换请求体。
- 假上游计数：`curl 127.0.0.1:18901/stats`（messages / chat / bytes），`curl -X POST 127.0.0.1:18901/stats/reset` 清零。

## 注意

- 日志：`/tmp/aidog-perf-<id>/{kernel,mock}.log`；隔离库 `/tmp/aidog-perf-<id>/.aidog/{aidog,platform,log}.db`。
- proxy_log 分组列是 `group_key`（不是 CLAUDE.md 里写的 `group_name`）。
- 内核启动会写 `$HOME/.claude*`、`.codex`、`.pi`（已被隔离到 /tmp），并每小时尝试 registry 联网同步、跑定时备份——测 CPU 时是背景噪声。
- 每个请求结束都会触发一次余额校准 `calibrate_from_quota`（即使 quota_source=manual），对本地 base_url 立即报 `Unsupported base_url` 返回、不联网；审计第 2 条的 TLS / 403 成本在此环境**复现不出来**。
