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
