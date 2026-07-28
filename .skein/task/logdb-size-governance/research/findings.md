# log.db 8.7GB 归因勘察

> 采集时间：2026-07-29。工具：`sqlite3 "file:~/.aidog/log.db?mode=ro"` 只读连接（未锁生产库）。

## 结论先行：登记时的「retention 疑失效」假设不成立

task 登记时（commit `40534848`）记的是「log.db 8.2GB/17338 行，retention 疑失效」。实测推翻这一归因 —— **retention 没失效，是根本还没到触发条件**。

## 实测数据

| 项 | 值 | 命令 |
|---|---|---|
| `PRAGMA auto_vacuum` | **2 (incremental)** | `PRAGMA auto_vacuum` |
| 总行数 | 18052 | `SELECT count(*) FROM proxy_log` |
| proxy_log 表体积 | 8681.5 MB | `SELECT sum(pgsize) FROM dbstat WHERE name='proxy_log'` |
| 全库文件 | 8687.7 MB | `ls -lh ~/.aidog/log.db` |
| 四个 body 类列合计 | **7115.3 MB（占表 82%）** | `sum(length(response_body)+length(request_body)+length(upstream_response_headers)+length(upstream_request_body))` |
| 单行最大 `response_body` | 1.4 MB | `max(length(response_body))` |

### 按日分布（`request_body` + `response_body` 两列）

| 日期 | 行数 | MB |
|---|---|---|
| 2026-07-29 | 726 | 128.2 |
| 2026-07-28 | 11320 | 2024.3 |
| 2026-07-27 | 6027 | 1489.3 |

全部数据仅跨 **3 天**。

## 为什么 retention 不该触发

`run_retention_cleanup`（`proxy_cmd/proxy_log.rs:138-156`）跑 4 步链，各自阈值：

| 步 | 默认阈值 | 本库最老数据距阈值 |
|---|---|---|
| `cleanup_user_request_fields` | 7 天（`models/proxy_log.rs:280`） | 差 4 天 |
| `cleanup_upstream_request_fields` | 7 天（`:281`） | 差 4 天 |
| `cleanup_proxy_logs`（删整行） | 90 天（`:282`） | 差 87 天 |
| `purge_deleted_proxy_logs` + `incremental_vacuum` | 无阈值，但硬删后为 no-op | — |

清理链逻辑完整、`auto_vacuum=incremental` 已正确开启（`incremental_vacuum` 不是死代码）。**所有行都在保留窗口内，不清是正确行为。**

## 真正的根因：单请求 body 落盘体积

平均每行 body ≈ 400 KB（7115 MB ÷ 18052 行）。50 路并发压测时流式响应正文全量入库，一天写进 2 GB。

按 CLAUDE.md 的既有设计，body 类列受 `log_user_request` / `log_upstream_request` 两个开关控制（gate 在 `gateway/models/proxy_log.rs` 的 `from_log` + `gateway/db/proxy_log.rs`）。开关开着时全文入库，**入库前无体积上限**。

## 治理方向（planning 时的候选，未拍板）

1. **入库前 body 体积 cap** —— 超阈值截断 + 标记「已截断」。与 memory `symmetric-body-cap` 呼应：流式/非流式两分支的 cap 必须对称，否则漏点。
2. **流式响应只留摘要** —— 流式已有 `"[stream]"` 占位控制标记的先例（终态判定依赖它，strip 时保留）。
3. **压测期临时关 `log_upstream_request`** —— 治标，但对本批性能量测立即有效，且零代码改动。
4. **retention 调参** —— ⚠️ 优先级最低。本次实测证明它不是本轮 8.7GB 的成因，调它不解决问题。

## 对其他 task 的影响

- `sqlite-page-cache-residency`：8.7GB 库 × `cache_size=-2000`（2MB/连接）的命中率假设需要复核 —— 该 task 的 design 是按小库估的。
- `perf-final-verification`：量测前应确认 log.db 状态，8.7GB 库的查询开销会混进 CPU 读数。
