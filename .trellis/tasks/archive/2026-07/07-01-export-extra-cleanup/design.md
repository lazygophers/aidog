# Design — 导出 extra 清洗 + obj 化

## 核心决策: collect 阶段 transform (不改 Platform struct serde)

Platform Rust struct 不动 (字段类型不变, DB schema 不动)。导出 collect 阶段把 `Platform` 转成中间 `ExportPlatform` struct, 在该层做:
- extra: `String` → `serde_json::Value` (parse 后 obj 序列化; parse 失败/空 → None skip)
- 运行时 7 字段 + status + enabled: 不进 ExportPlatform (白名单省略)
- 配置空值: `skip_serializing_if`

## 改动点

### 1. `collect.rs` — 导出 transform
```rust
// 新建 ExportPlatform (collect 内部或 models 模块)
#[derive(Serialize)]
struct ExportPlatform {
    id: u64,
    name: String,
    platform_type: Protocol,
    base_url: String,
    api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<serde_json::Value>,  // 空 → None (省略); 非空 → obj
    #[serde(skip_serializing_if = "PlatformModels::is_empty")]
    models: PlatformModels,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    available_models: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    endpoints: Vec<PlatformEndpoint>,
    created_at: i64,
    updated_at: i64,
    // 不含: status/enabled/auto_*/expires_at/deleted_at/est_*/last_real_query_at
}

// Platform → ExportPlatform
fn to_export(p: Platform) -> ExportPlatform {
    let extra = if p.extra.trim().is_empty() || p.extra.trim() == "{}" {
        None
    } else {
        serde_json::from_str(&p.extra).ok()  // string → obj; 失败 → None (兜底)
    };
    ExportPlatform { id: p.id, name: p.name, ..., extra, ... }
}
```
`payload.platform = platforms.into_iter().map(to_export).collect::<Result<...>>()` (collect.rs:37-41 替换)。

### 2. `apply/db_rows.rs` — 导入兼容 obj + 缺失字段
- `effective_extra_with_breaker` (L160): 当前 `json_str(row,"extra")` 按 string 取。改: 先看 extra 是 Object → stringify 后走 breaker parse; 是 String → 原样; 缺失 → 空字符串。
- 运行时/status/enabled 缺失: Platform 反序列化已有 `#[serde(default)]` 覆盖大多 (status/endpoints/auto_*/expires_at/deleted_at/est_*/last_real_query_at)。**enabled 无 default → 补 `#[serde(default)]`** (platform.rs)。导入后 enabled=false, status=default(Enabled?) — 确认 PlatformStatus::default。

### 3. `ImportExport.tsx` — preview 显示
- extra 已是 preview 渲染, 现数据源从 string 变 obj (后端 collect 改后 preview 自动反映). 确认无裸 `"{}"` 显示; 若 preview 有 string 展示逻辑, 调整为 obj 展开。

## 导入兼容矩阵
| 来源 | extra 形态 | 处理 |
|---|---|---|
| 旧 .aidogx (全字段) | `"{}"` string | json_str 原样 → DB; breaker 走 string parse |
| 旧 .aidogx (有 breaker) | `"{\"breaker\":...}"` string | 同上 |
| 新 .aidogx (清洗) | `{...}` obj 或缺失 | Object → stringify → DB; 缺失 → 空 |
| 新非空 extra | `{"mock":...}` obj | stringify → DB |

## 验证断言
- 导出: `cargo test -p aidog test_collect` 新增断言 — 空 extra 平台无 extra 字段; 非空 obj; 无运行时字段
- 导入: `test_db_rows` 新增 — obj extra + 缺失字段可导入, breaker 迁入兼容 obj
- `test_conflicts` fixture extra `"{}"` string 仍兼容

## 非目标重申
- DB schema / Platform Rust 字段类型不动
- mock/newapi/breaker 配置逻辑不动
