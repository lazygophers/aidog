# Design — Group 智能调度与熔断器

> 架构契约。GA(后端)/GB(前端) 据此落地。PRD 见 `prd.md`，调研见 `research/reference-repo-scheduling-breaker.md`。

## 数据模型（契约 — Rust ↔ TS 字段名一致）

### Platform 新增字段（熔断配置，0=继承全局默认）
```rust
// models.rs Platform struct 追加
pub breaker_failure_threshold: u32,  // 0=继承
pub breaker_open_secs: u64,          // 0=继承
pub breaker_half_open_max: u32,      // 0=继承
```
db.rs：Platform 表加 3 列（migration），默认 0。

### RoutingMode 新增变体（models.rs 枚举）
```rust
enum RoutingMode { /* 现有 */ LoadBalance, Failover, ...,
  HealthAware,   // 熔断摘除 + 加权随机（健康平台中加权）
  LeastLatency,  // 按 per-platform 延迟 EMA 升序
  Sticky,        // session→platform 绑定，回退加权随机
}
```
serde rename 与 TS 字面量对齐。

### SchedulingBreakerSettings（全局默认，settings KV scope=`scheduling`）
```rust
pub struct SchedulingBreakerSettings {
  pub default_routing_mode: String,      // 默认调度策略
  pub breaker_failure_threshold: u32,    // default 5
  pub breaker_open_secs: u64,            // default 1800
  pub breaker_half_open_max: u32,        // default 2
  pub enabled: bool,                     // 熔断总开关 default true
}
```

### 熔断状态机 + 指标（内存，新文件 scheduling.rs 或 router 内）
```rust
enum BreakerState { Closed{fails:u32}, Open{until_ms:i64}, HalfOpen{probes:u32} }
struct PlatformHealth { breaker: BreakerState, latency_ema_ms: f64, inflight: u32 }
// SchedulerState: RwLock<HashMap<platform_id(u64), PlatformHealth>>，随 ProxyState/独立单例持有
// 解析有效阈值: platform 字段非 0 用之，否则全局默认
```

## 熔断状态机转移
```
Closed: 记 fail（5xx/超时，retry 耗尽才计一次）→ fails+1；达 threshold → Open{until=now+open_secs}。成功 → fails=0。
Open:   候选过滤直接踢出；now>=until → 转 HalfOpen{probes=0}。
HalfOpen: 放行至多 half_open_max 个探测；任一成功 → Closed；任一失败 → Open{重置 until}。
不计熔断: 401/403(走 auto_disabled)、客户端 4xx(非 429)、probe 请求。
```

## 候选过滤与调度（router.rs select_candidates / order_load_balance）
```
1. 现有: 按 priority 分层 + active/probe 分桶。
2. 新增准入门: 过滤掉 [熔断 Open] ∪ [auto_disabled] 的平台（二者独立判定，取并集跳过）。HalfOpen 平台限量放行。
3. 按 Group.routing_mode 排序生效平台:
   - LoadBalance(现有): 加权随机
   - Failover(现有): priority 升序
   - HealthAware: 健康平台中加权随机（= 准入门已摘 Open，等价加权随机 on 健康集）
   - LeastLatency: latency_ema_ms 升序
   - Sticky: session 键查绑定平台(若健康) 否则回退加权随机 + 写绑定
4. routing_mode 为空/未识别 → 全局默认 default_routing_mode。
```

## proxy.rs 接线（重试循环）
```
每次 forward 尝试: 
  inflight+1（指标）
  结果 → recordSuccess(platform_id, latency) / recordFailure(platform_id)
    success: 更新 latency_ema、breaker Closed/HalfOpen→Closed、inflight-1
    failure(5xx/超时，且本平台 retry 耗尽): breaker fail 计数、inflight-1
  401/403: 现有 auto_disabled 逻辑不变（熔断不介入）
```
recordFailure 时机 = 对该候选平台尝试失败切换前（参照调研 forwarder「重试耗尽才 recordFailure」语义；aidog 现有重试是换候选平台，故每候选失败即计一次）。

## Sticky session 键
- 优先复用现有 session 概念（grep proxy.rs/models.rs 是否有 session_id）；无则按客户端标识（如 group_name + 请求中稳定标识）。MVP 可用内存 LRU<session_key, platform_id> + TTL。

## 契约冻结（GA 产出，GB 消费）
GA 在 api.ts 写：Platform 类型加 3 个 breaker 字段；RoutingMode 字面量加 3 变体；`SchedulingBreakerSettings` 类型 + `schedulingApi.{getSettings,setSettings}`（Platform/Group CRUD 复用现有 platformApi/groupApi，仅扩字段）。GB 消费不改契约。

## 与中间件树解耦
本树 MVP 直接按 HTTP status/timeout 判定熔断，不强依赖 middleware 树 error_rule。若 error_rule 信号可用（C3），HealthAware 可选消费其 non-retryable 标记加速摘除（增强项，非 MVP 必须）。

## 资源边界 / 跨树串行
GA 改 proxy.rs/models.rs/router.rs/db.rs/lib.rs/api.ts → 与 middleware 后端 child（C2/C3/C4）**全局后端串行**。GB 改前端，GA 冻结契约后并行（但与 C5 同改 api.ts/AppSettings/Platform·Group 编辑页 → GB 与 C5 前端串行）。

## Rollback
全局默认调度=现有模式、合理熔断阈值；Platform 空=继承。熔断总开关 enabled=false 旁路。worktree 隔离。
