# S2 — 读写分类清单（读站点迁移 read pool 依据）

判定规则：
- **read**（迁 `call_read_traced`）：纯 SELECT，无写副作用，无 `last_insert_rowid`/`RETURNING`，UI 卡顿相关。
- **write**（保留 `call_traced`）：INSERT/UPDATE/DELETE/DDL/PRAGMA、含写副作用、`invalidate_*`、事务内写、写后回读（`last_insert_rowid`/同闭包 SELECT 验证）、含写工作流中间步骤（保守留写）。

覆盖候选文件全部 `call_traced` 站点。行号对应迁移前。

## query_stats.rs
| 行 | 方法 | 判定 | 判据 |
|---|---|---|---|
| 57 | query_stats | **read** | 纯 SELECT（query_stats_inner 仅查 proxy_log），Stats 页热查询 |
| 75 | query_stats_batch | **read** | 批量纯 SELECT，浮窗 N 卡聚合 |

## stats_today.rs
| 57 行起 | 方法 | 判定 | 判据 |
|---|---|---|---|
| 20 | today_token_total | **read** | 纯 SELECT proxy_log（仅 `#[cfg(test)]`，迁移无害） |
| 66 | today_stats | **read** | 纯 SELECT stats_agg_hourly，托盘/浮窗热读 |
| 129 | today_platform_stats | **read** | 纯 SELECT stats_agg_hourly + platform，浮窗热读 |

## stats_agg.rs（不迁，全写/聚合）
| 75 | upsert_stats_agg | write | UPSERT 写聚合表 |
| 135 | rebuild_stats_agg_from_logs | write | DELETE + INSERT 重建 |
| 180 | cleanup_stats_agg | write | UPDATE 软删 |

## usage_stats.rs（全 read）
| 81 | get_platform_usage_stats | **read** | 纯 SELECT proxy_log 聚合，平台卡片用量 |
| 104 | get_last_test_result | **read** | 纯 SELECT proxy_log，平台卡片测试徽章 |
| 150 | get_group_usage_stats | **read** | 纯 SELECT stats_agg_hourly，Groups 页 |
| 205 | get_all_group_usage_stats | **read** | 批量纯 SELECT，Groups 页 N+1 消除 |
| 270 | platform_usage_stats_all | **read** | 批量纯 SELECT，Platforms 页 |
| 444 | get_group_hourly_rate | **read** | 纯 SELECT stats_agg_hourly，statusline 配色 |
| 464 | get_platform_hourly_rate | **read** | 纯 SELECT stats_agg_hourly，Platforms 配色 |

## proxy_log.rs
| 54 | upsert_proxy_log | write | INSERT OR REPLACE |
| 208 | insert_proxy_log_columns | write | INSERT |
| 236 | update_proxy_log_columns | write | UPDATE |
| 260 | list_proxy_logs | **read** | 纯 SELECT，Logs 页列表 |
| 305 | filtered_list_proxy_logs | **read** | 纯 SELECT，Logs 页过滤列表 |
| 332 | filtered_count_proxy_logs | **read** | 纯 SELECT COUNT，Logs 页分页 |
| 408 | get_proxy_log | **read** | 纯 SELECT 单行，Logs 详情 |
| 424 | clear_proxy_logs | write | UPDATE 软删 |
| 445 | cleanup_proxy_logs | write | DELETE + vacuum + ANALYZE |
| 470 | purge_deleted_proxy_logs | write | DELETE + vacuum |

## group.rs
| 56 | create_group | write | INSERT + last_insert_rowid + invalidate |
| 103 | reorder_groups | write | UPDATE + invalidate |
| 126 | reorder_platforms | write | UPDATE + invalidate |
| 155 | reorder_group_platforms | write | UPDATE + invalidate |
| 188 | set_group_platform_level_priority | write | UPDATE + invalidate |
| 218 | move_group_platform | write | DELETE+INSERT 事务 + invalidate |
| 262 | list_groups | **read** | 纯 SELECT（缓存未命中回源），resolve/Groups 热读 |
| 281 | get_group | **read** | 纯 SELECT 单行 |
| 311 | update_group | write | UPDATE + invalidate |
| 344 | set_default_group | write | UPDATE + invalidate |

## group_platform.rs
| 10 | force_delete_group | write | UPDATE 软删 + invalidate |
| 34 | set_group_platforms | write | DELETE+INSERT + invalidate |
| 74 | sync_platform_manual_groups（列当前关联）| **write（保守）** | 单条 SELECT，但处于多步写工作流首步（后续按结果 DELETE/INSERT），保守留写连接，避免读写工作流跨连接语义疑虑 |
| 98 | sync_platform_manual_groups（移出 DELETE）| write | DELETE + invalidate |
| 146 | get_group_platforms | **read** | 纯 SELECT JOIN，Groups 页 / get_group_detail 热读 |

## platform.rs
| 70 | create_platform | write | INSERT + last_insert_rowid + invalidate |
| 123 | list_platforms | **read** | 纯 SELECT，Platforms 页列表 |
| 139 | get_platform | **read** | 纯 SELECT 单行 |
| 230 | update_platform | write | UPDATE + invalidate |
| 289 | set_platform_auto_disabled | write | SELECT+UPDATE 事务 + invalidate |
| 331 | recover_platform_auto_disabled | write | UPDATE + invalidate |

## model_price.rs
| 51 | list_model_prices | **read** | 纯 SELECT，Pricing 页列表 |
| 72 | count_model_prices | **read** | 纯 SELECT COUNT |
| 87 | get_model_price | **read** | 纯 SELECT（manual→github 两查），resolve_price/estimate 热读 |
| 124 | upsert_model_price | write | UPSERT |
| 283 | search_model_prices | **read** | 纯 SELECT LIKE |
| 313 | filtered_list_model_prices | **read** | 纯 SELECT 过滤 |
| 369 | filtered_count_model_prices | **read** | 纯 SELECT COUNT |

## mcp.rs
| 8 | list_mcp_servers | **read** | 纯 SELECT，MCP 管理列表 |
| 47 | get_mcp_server | **read** | 纯 SELECT 单行 |
| 83 | upsert_mcp_server | write | UPSERT |
| 117 | delete_mcp_server | write | DELETE |
| 136 | set_mcp_server_enabled_agents | write | UPDATE |
| 152 | list_mcp_server_names | **read** | 纯 SELECT |

## middleware.rs
| 49 | list_middleware_rules | **read** | 纯 SELECT，引擎 reload + 前端列表 |
| 72 | create_middleware_rule | write | INSERT + last_insert_rowid + 回读 |
| 118 | update_middleware_rule | write | UPDATE + 回读 |
| 163 | delete_middleware_rule | write | DELETE |
| 215 | insert_notification | write | INSERT + last_insert_rowid |
| 236 | list_notifications | **read** | 纯 SELECT，收件箱列表 |
| 262 | clear_notifications | write | DELETE |
| 282 | cleanup_notifications | write | DELETE + vacuum |

## settings.rs
| 26 | get_setting | **read** | 纯 SELECT（缓存未命中回源），热读 |
| 59 | set_setting | write | UPSERT + invalidate |
| 81 | delete_setting | write | UPDATE 软删 + invalidate |
| 101 | list_setting_keys | **read** | 纯 SELECT |
| 117 | list_all_settings_raw | **read** | 纯 SELECT，导出用 |
| 139 | list_all_group_platform_pairs | **read** | 纯 SELECT JOIN，导出用 |

## 迁移汇总（read 站点 → call_read_traced）
- query_stats.rs: 57, 75
- stats_today.rs: 20, 66, 129
- usage_stats.rs: 81, 104, 150, 205, 270, 444, 464
- proxy_log.rs: 260, 305, 332, 408
- group.rs: 262, 281
- group_platform.rs: 146
- platform.rs: 123, 139
- model_price.rs: 51, 72, 87, 283, 313, 369
- mcp.rs: 8, 47, 152
- middleware.rs: 49, 236
- settings.rs: 26, 101, 117, 139

共 **35 个读站点**迁 read pool；其余写/含写副作用站点保留写连接。
group_platform.rs:74 纯 SELECT 但保守留写（写工作流首步）。
</content>
</invoke>
