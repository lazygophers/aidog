# 路由算法

## 分组匹配

请求到达后，按以下优先级匹配分组：

```
for group in groups:
  if request.model in group.model_mapping:
    return group
return default_group || groups[0]
```

## 模型映射链

```
resolved_model = request.model

# 1. 分组级映射
if group.model_mapping.contains(resolved_model):
  resolved_model = group.model_mapping[resolved_model]

# 2. 平台级映射
if platform.model_mapping.contains(resolved_model):
  resolved_model = platform.model_mapping[resolved_model]

# 3. 使用 resolved_model
```

## 平台选择

### 顺序策略
```rust
for platform in group.platforms:
  if platform.enabled && platform.api_key_valid:
    return platform
```

### 随机策略
```rust
let enabled: Vec<_> = group.platforms.iter()
  .filter(|p| p.enabled && p.api_key_valid)
  .collect();
return enabled[rand() % enabled.len()];
```

### 加权策略
```rust
// 加权随机选择
let total_weight: f64 = enabled.iter().map(|p| p.weight).sum();
let mut r = rand() * total_weight;
for platform in enabled:
  r -= platform.weight;
  if r <= 0:
    return platform
```

## 故障转移

```
let platforms = select_by_strategy(group);
for platform in platforms:
  match send_request(platform, request):
    Ok(response) => return response
    Err(429 | 5xx | Timeout) => continue  // 尝试下一个
    Err(_) => return error
return Error("All platforms failed")
```

## 分组-平台关联

分组和平台是多对多关系，通过 `group_platforms` 关联表管理：
- 排列顺序决定优先级（顺序策略）
- 权重字段用于加权策略
- 每个关联可有独立的模型映射覆盖

## 候选过滤维度（candidate_state）

`candidate_state(platform, now_ms)` 按以下正交维度判定平台是否纳入候选（任一命中即排除，互不覆盖）：

1. **过期**（`expires_at > 0 && now_ms >= expires_at`）：等效自动禁用，但独立于 status 三态；改值清空/延后即恢复。
2. **高峰禁用**（`platform.extra.disable_during_peak == true && is_in_peak_window(peak_hours, now_ms)`）：临时闸门，不改 status；关开关 / 出窗口即恢复。命中 `peak_hours` 任一窗口（first-match，与 `resolve_multiplier` 同 hit 逻辑）即排除。
3. **status 三态**：Enabled 纳入；AutoDisabled 已过退避时间纳入末尾试探；Disabled（手动）/ AutoDisabled 未到退避 排除。

### 高峰禁用语义细节

- **per-platform 开关**：`platform.extra.disable_during_peak`（bool，默认 false）；preset 可给 per-protocol 默认。判定 helper：`gateway::router::is_peak_disabled`（复用 `peak_hours::is_in_peak_window`）。
- **单平台组不 bypass**：此开关优先级高于 status bypass（status 维度照旧 bypass auto_disabled / 熔断，必请求），单平台组高峰期请求直接 `Err("peak_disabled")` → 503。
- **整组全被高峰排除**：`select_candidates_ctx` 返 `Err("peak_disabled")`（区别于普通 NoCandidate 的 `"no available platform..."`）→ `proxy/handler.rs` route fail 路径落 `proxy_log(blocked_by='router', blocked_reason='peak_hours', status_code=503, est_cost=0)` 审计行；其他 NoCandidate 原因照旧 warn 不落库。
- **前端**：`utils/peakHours.ts` `isCurrentlyPeak(windows, nowMs)` 与 Rust `is_in_peak_window` 逐行对称（cross-layer 一致）。PlatformCard 徽标（列表 + Groups 共享）+ formSections 编辑表单「当前: 高峰/非高峰」实时态。
