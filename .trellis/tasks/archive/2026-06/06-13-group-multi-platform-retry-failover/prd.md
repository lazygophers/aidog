# PRD: 分组多平台请求重试 + 自动禁用 + 尝试记录

## 背景

分组有多平台时, 当前 (research/01-04):
- `router.rs:13-80` `select_platform` 整请求**只选 1 个平台**, 无候选列表; `Failover` (router.rs:111-120) 是"按 priority 静态选第一个 enabled", 非运行时失败切换。
- `proxy.rs:644` 选 1 平台; 上游非 2xx(含 401/403) **直接透传 return, 无重试/无换平台/无 401/403 检测** (proxy.rs:842-885)。
- 平台状态 = **`bool enabled`** (models.rs:340, DB `enabled INTEGER 0/1`), 仅两态。
- **一请求 = 一行 proxy_log** (同 id INSERT OR REPLACE 渐进更新), `platform_id` 单值, **无法记多次尝试**。

## 目标

分组多平台时实现: ① 失败逐个重试(可设最大次数) ② 401/403 自动禁用平台 + 自动恢复 ③ 记录每次尝试并在列表/详情展示。

## 决策 (已确认)

1. **最大重试次数**: 分组级 (group 加 `max_retries` 字段)。
2. **平台状态**: `bool enabled` → 三态 `status` enum: `enabled` / `disabled`(用户手动禁用) / `auto_disabled`(401/403 自动)。
3. **自动禁用恢复**: 手动重启用 + 改 api_key 自恢 + 定时指数退避自动试探(基础 1h)。
4. **尝试记录**: 单行 proxy_log + `attempts` JSON 列 (每次记平台/状态码/耗时/错误)。
5. **流式重试**: 基于响应头状态码, 首 chunk 转发前可重试; 已开始转发 body 不重试。

## 数据模型

### platform 表 (migration: 加列, 从 enabled 迁移)
- `status TEXT NOT NULL DEFAULT 'enabled'` — enabled/disabled/auto_disabled。migration: `enabled=0 → 'disabled'`, 否则 `'enabled'`。
- `auto_disabled_until INTEGER NOT NULL DEFAULT 0` — 下次试探时间 (unix ms), 退避用。
- `auto_disable_strikes INTEGER NOT NULL DEFAULT 0` — 连续自动禁用次数, 指数退避指数。
- `enabled` 列**保留**(向后兼容旧读者), 写入端同步: `status=='enabled' → enabled=1 else 0`。**router 过滤改用 status**。
- models.rs Platform: `enabled: bool` 旁加 `status: PlatformStatus` enum + 两个退避字段; TS 类型同步。

### group 表
- `max_retries INTEGER NOT NULL DEFAULT 2` — 分组级最大重试次数 (0=不重试只试 1 次)。models.rs Group + TS + Groups.tsx 编辑字段。

### proxy_log 表
- `attempts TEXT NOT NULL DEFAULT '[]'` — JSON 数组, 每元素 `{platform_id, platform_name, status_code, error, duration_ms, ts}`。
- `retry_count INTEGER NOT NULL DEFAULT 0` — = attempts.len()-1 (0 表示一次成功)。
- 现有 `platform_id` = **最终成功(或最后尝试)平台**。

## 重试编排 (proxy.rs)

```
candidates = router.select_candidates(group)   // 新增: 返回有序候选列表
    // 排序: failover 按 priority; load_balance 按权重/轮询
    // 过滤: status==enabled, 或 (status==auto_disabled 且 now>=auto_disabled_until) 试探纳入
max = group.max_retries
attempts = []
for (i, platform) in candidates.enumerate():
    if i > max: break                            // 超过最大重试次数
    resp = forward(platform, req)
    record_attempt(attempts, platform, resp)     // 平台/状态码/耗时/错误
    match resp:
        2xx → 若该平台曾 auto_disabled 则恢复(status=enabled, 清 strikes/until); break 成功
        401/403 → mark_auto_disabled(platform); continue   // 换下个候选
        其他错误(5xx/超时/连接失败) → continue              // 换下个候选(也算重试)
若无候选 / 全部失败 → 返回最后一次错误
最终: proxy_log.platform_id=最终平台, attempts=记录, retry_count=len-1
```

- **流式**: forward 在收到响应头(状态码)阶段判定; 2xx 开始转发 body 前不再可换平台; 401/403/5xx 在头阶段触发重试。复用流式日志 StreamAggregator 不冲突。

## 自动禁用 + 恢复

- **触发**: 401/403 → `status=auto_disabled`, `auto_disable_strikes++`, `auto_disabled_until = now + 1h * 2^(strikes-1)` (指数退避: 1h/2h/4h/8h...)。
- **定时试探**: router 选候选时, `auto_disabled` 且 `now >= auto_disabled_until` → 纳入候选末尾试探。成功 → 恢复 `enabled` 清 strikes/until; 再 401/403 → strikes++ 退避延长。
- **手动恢复**: Platforms 页对 auto_disabled 平台"重新启用" → `status=enabled` 清 strikes/until。
- **改凭证自恢**: `platform_update` 检测 api_key 变化 → 若当前 auto_disabled 则清为 enabled + 清 strikes/until。

## 前端

- **Platforms.tsx**: 状态三态展示(enabled 绿 / disabled 灰 / auto_disabled 橙+提示"401/403 自动禁用, 下次试探 <时间>"); toggle 逻辑适配三态(auto_disabled 可手动→enabled)。
- **Groups.tsx**: group 编辑表单加 `max_retries` 数字字段。
- **Logs.tsx**: 列表行显示最终平台名 + 重试次数徽标(retry_count>0 时); 详情展开 attempts 每次尝试(平台/状态码/耗时/错误, 时序列表)。
- i18n: 新文案走 7 语言 key。

## 范围

后端: models.rs(PlatformStatus enum + Platform/Group 字段) / db.rs(migration + CRUD + upsert attempts) / migrations/(新文件) / router.rs(select_candidates 候选列表) / proxy.rs(重试循环 + 401/403 检测 + 自动禁用联动 + attempts 记录) / lib.rs(platform_update 改凭证自恢 + 状态切换 command)。
前端: api.ts(类型) / Platforms.tsx / Groups.tsx / Logs.tsx / locales。

## 非目标

- 不改非分组(单平台)请求路径的成功逻辑(单平台无候选可换, 仍记 attempt)。
- 不引入后台常驻调度线程做恢复(用"重试时惰性试探"实现定时恢复, 零额外线程)。
- 不改现有 routing_mode 语义(failover/load_balance 仅作候选排序依据)。

## 验收标准

- 分组多平台: 首选失败(5xx/超时)自动重试下一平台, 直到成功或候选耗尽或超 max_retries。
- 平台返 401/403 → status 变 auto_disabled, 退避时间正确(1h/2h/4h 指数), 不影响用户手动 disabled 的平台。
- auto_disabled 平台过退避时间被重试惰性试探; 成功恢复 enabled; 改 api_key 立即恢复。
- proxy_log 记 attempts(每次平台/状态码/耗时); 列表显示最终平台+retry_count; 详情显示每次尝试。
- 流式请求: 头阶段 401/403 可重试, 已转发 body 不重试。
- 迁移: 现有 enabled=0 平台迁移为 status=disabled, 不误判为 auto_disabled。
- cargo build + cargo test + yarn tsc 0 error 无新增 warning; 7 语言 i18n 齐。
- Rust↔TS 契约一致(status enum / attempts / max_retries / retry_count)。

## 编排

单一交付, 单 worktree。改动跨后端(schema/router/proxy/状态机)+ 前端(三页)+ i18n, 但**强耦合**: 共享 status enum / attempts / max_retries 契约, 前端消费后端字段, proxy.rs 重试循环为不可分核心。不拆 child。main 派 1 个 trellis-implement 在 worktree 内按"后端数据模型+迁移 → router 候选 → proxy 重试循环+自动禁用 → lib command → 前端三页 → i18n"串行实施。
