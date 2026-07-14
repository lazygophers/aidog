# CPA 解析器 OAuth 凭据识别 — 设计

## 改动面
仅 `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser.rs` 单文件 3 处逻辑 + 测试。

## A. parse_single_file 加 OAuth 识别分支
当前(parser.rs:169-214): 读 content → 按扩展名 yaml/json 反序列化 `CpaConfigStub` → `is_cpa_config()` 判定 → 无段则 Err。

改: JSON 扩展名时, **先**试 OAuth 识别, 命中返 OAuth provider; **否则**走原 CpaConfigStub 流程。

```rust
fn parse_single_file(path: &Path) -> Result<Vec<CpaProvider>, String> {
    let content = fs::read_to_string(path)...;
    let ext = path.extension()...;

    if ext == "yaml" || ext == "yml" {
        // YAML 不试 OAuth(CLIProxyAPI OAuth 凭据均 JSON), 直走 CPA config
        ... 原 CpaConfigStub 流程不变 ...
    } else if ext == "json" {
        // 先试 OAuth 凭据(JSON 含 type+access_token)
        if let Some(oauth_providers) = parse_oauth_json(&content) {
            return Ok(oauth_providers);  // 单文件单凭据, Vec 长度 0 或 1
        }
        // 否则走 CPA config
        ... 原 CpaConfigStub 流程不变 ...
    } else {
        return Err(...);
    }
}
```

## B. 抽 parse_oauth_json 复用
当前 `scan_auth_dir`(parser.rs:546-571) 内联 OAuthCredential 反序列化 + provider 构造。抽公共函数:

```rust
/// 解析 JSON 内容为 OAuth provider(单文件/单凭据)。非 OAuth 凭据返 None。
fn parse_oauth_json(content: &str) -> Option<Vec<CpaProvider>> {
    let cred: OAuthCredential = serde_json::from_str(content).ok()?;
    let oauth_type = CpaOAuthType::parse_oauth_type(&cred.cred_type)?;
    let access_token = cred.access_token?;  // 无 token → None(交回 CPA config 流程)
    let models = cred.model_aliases.unwrap_or_default().into_iter().map(|m| m.name).collect();
    Some(vec![CpaProvider {
        source_segment: CpaSourceSegment::OAuth,
        name: cred.email.clone(),
        base_url: String::new(),
        api_key: access_token,
        models,
        prefix: None,
        headers: HashMap::new(),
        disabled: false,
        oauth_type: Some(oauth_type),
    }])
}
```

`scan_auth_dir` 改调 `parse_oauth_json(&content)`(删内联逻辑)。

**注意 None 语义**: `parse_oauth_json` 返 None = 「不是 OAuth 凭据或无 access_token」。parse_single_file 收到 None 继续走 CPA config stub(容错: 极端情况某 JSON 既非 OAuth 又无 CPA 段 → 仍走原 Err「无任何 CPA provider 段」)。

scan_auth_dir 收到 None = 跳过该文件(同当前行为)。

## C. deduplicate_providers OAuth 段改 key
当前(parser.rs:639-649) OAuth 段 dedup key = `(source_segment, base_url)`, base_url 空全撞。

改: OAuth 段 dedup key = `(source_segment, name_or_email)`:

```rust
} else if provider.source_segment == CpaSourceSegment::OAuth {
    // OAuth 按 (oauth_type, name/email) 去重; 各凭据 email 不同 = 各自独立
    let key_name = provider.name.clone().unwrap_or_default();
    if !seen_keys.insert((provider.source_segment, key_name)) {
        ... merge_models ... continue;
    }
    ...
} else {
    // 其他 api-key 段按 (segment, base_url) 不变
    ...
}
```

**不变量**: OAuth provider 各凭据 email 不同(CLIProxyAPI 多账号语义)→ 各自独立 dedup key 不撞。

## 测试
1. `test_parse_oauth_json_single` — `{"type":"xai","email":"a@b","access_token":"tok","model_aliases":[{"name":"grok-1","alias":"g1"}]}` → 1 provider(xai, name=a@b, models=[grok-1])
2. `test_parse_oauth_json_no_token` — `{"type":"xai"}` → None
3. `test_parse_oauth_json_unknown_type` — `{"type":"unknown","access_token":"tok"}` → None
4. `test_dedup_oauth_distinct_emails` — 2 个 xai OAuth(name=a@b / c@d) → 2 provider 不合并
5. 回归: `test_parse_models` / `test_deduplicate_providers`(openai-compat name 合并)不变

## 不改
- mapper.rs(OAuth 映射正确)
- cpa_import.rs 命令签名
- 前端 ParseResult 结构
- scan_auth_dir 公开行为(仅复用 parse_oauth_json)

## D. W2 修复 — 损坏 OAuth 独立错误文案(parse_single_file 内)
用户 grill 拍板补 W2。当前 parse_oauth_json 无 access_token 返 None → parse_single_file 走 CPA config stub → Err「无 CPA provider 段」误导(实际是 OAuth 缺 token)。

加探测函数 + parse_single_file JSON 分支区分文案:

```rust
/// 探测 JSON 是否为 OAuth 凭据(仅看 type 可否 parse 为 CpaOAuthType, 不要求 access_token)。
fn is_oauth_credential(content: &str) -> bool {
    #[derive(Deserialize)]
    struct TypeProbe { #[serde(rename = "type")] t: String }
    serde_json::from_str::<TypeProbe>(content)
        .ok()
        .and_then(|p| CpaOAuthType::parse_oauth_type(&p.t))
        .is_some()
}
```

parse_single_file JSON 分支:
```rust
} else if ext == "json" {
    if let Some(oauth_providers) = parse_oauth_json(&content) {
        return Ok(oauth_providers);
    }
    // 是 OAuth 凭据但 parse_oauth_json None = 缺 access_token → 独立文案
    if is_oauth_credential(&content) {
        return Err("OAuth 凭据缺少 access_token".to_string());
    }
    // 走 CPA config stub(原逻辑)
    ...
}
```

scan_auth_dir 不动(无 token 跳过同当前行为, auth_dir 扫描静默跳过合理)。

补测试:
6. `test_parse_single_file_oauth_missing_token` — 写临时 JSON `{"type":"xai"}` → parse_single_file Err「OAuth 凭据缺少 access_token」(非「无任何 CPA provider 段」)
