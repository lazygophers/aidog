# 数据库设计

## SQLite

数据库路径：`~/.aidog/aidog.db`

使用 `rusqlite` crate 操作。

## 主要表结构

### platforms

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增主键 |
| name | TEXT | 显示名称 |
| base_url | TEXT | API 基础地址（含版本前缀） |
| api_key | TEXT | API Key |
| protocol | TEXT | 协议类型 |
| enabled | BOOLEAN | 是否启用 |
| model_mapping | TEXT | JSON 格式模型映射 |

### groups

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增主键 |
| name | TEXT | 分组名称 |
| description | TEXT | 分组描述 |
| routing_strategy | TEXT | 路由策略 |
| is_default | BOOLEAN | 是否默认分组 |

### proxy_logs

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增主键 |
| timestamp | DATETIME | 请求时间 |
| model | TEXT | 模型名 |
| platform_id | INTEGER | 关联平台 |
| group_id | INTEGER | 关联分组 |
| status | TEXT | 成功/失败 |
| prompt_tokens | INTEGER | Prompt Token 数 |
| completion_tokens | INTEGER | Completion Token 数 |
| est_cost | REAL | 预估费用 |
| latency_ms | INTEGER | 延迟毫秒 |
| user_request | TEXT | 用户原始请求 |
| upstream_request | TEXT | 上游请求 |
| upstream_response | TEXT | 上游响应 |
| error_message | TEXT | 错误信息 |

### settings

键值对存储，JSON 格式保存复杂配置。

## Retention 清理策略

### 字段级清理（不清除行）
```sql
UPDATE proxy_logs SET user_request='', upstream_request='', upstream_response=''
WHERE timestamp < datetime('now', '-7 days');
```

### 行级清理
```sql
DELETE FROM proxy_logs WHERE timestamp < datetime('now', '-90 days');
```

## 统计聚合

today_stats 通过 SQL 聚合查询：
```sql
SELECT
  COUNT(*) as total_requests,
  SUM(prompt_tokens) as total_prompt_tokens,
  SUM(completion_tokens) as total_completion_tokens,
  SUM(est_cost) as total_cost
FROM proxy_logs
WHERE date(timestamp) = date('now');
```

Group 统计由前端聚合：取关联平台的 stats 求和。
