# Journal - nico (Part 1)

> AI development session journal
> Started: 2026-06-09

---



## Session 1: DB schema v2 规范化重构

**Date**: 2026-06-11
**Task**: DB schema v2 规范化重构
**Branch**: `master`

### Summary

10 条 DB 规范破坏式重构: 表名单数/uint64自增PK(proxy_log除外uuid去连字符)/ms时间戳/每表软删除deleted_at/禁NULL默认值/protocol→platform_type/复合表加代理PK/删model_mappings内联group JSON/独立一次性迁移脚本. 后端(models/db/router/proxy/lib)+前端(api.ts/pages)全对齐, schema测试11绿, 真库~/.aidog/aidog.db迁移成功, BUG-1(JOIN列歧义)修复, simplify质量重构8项. 后续: ORM评估/软删除统一封装/auto_from_platform改INTEGER FK.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `5bb8743` | (see git log) |
| `979f096` | (see git log) |
| `a05960a` | (see git log) |
| `48d3ebe` | (see git log) |
| `f9eaaed` | (see git log) |
| `60eebde` | (see git log) |
| `348ff28` | (see git log) |
| `c980dd7` | (see git log) |
| `ce26867` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 2: 添加 mock 平台类型

**Date**: 2026-06-11
**Task**: 添加 mock 平台类型
**Branch**: `master`

### Summary

新增 Protocol::Mock 平台类型: 路由到 mock 平台不转发真实上游, 本地按入站协议(anthropic/openai/openai_completions/openai_responses/gemini)生成可控假响应(非流式+流式SSE). 三层配置覆盖(请求body.mock>message role映射>platform.extra), error_mode(none/http_error/429/timeout) + delay_ms + 假token. 配置存platform.extra零schema变更. 前端MockConfigEditor. 拦截点proxy.rs handle_mock仅matches!(Mock)不影响现有平台. 22单测全绿, spec沉淀mock-platform.md.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `dae1c10` | (see git log) |
| `d3c2188` | (see git log) |
| `448570b` | (see git log) |
| `73fa042` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 3: Claude Code 订阅平台（纯透传）

**Date**: 2026-06-11
**Task**: Claude Code 订阅平台（纯透传）
**Branch**: `master`

### Summary

新增 Protocol::ClaudeCode 平台类型, 纯透传 relay: 路由到 CC 平台原样转发客户端请求到 base_url, 不转换 body/header/不注入认证(客户端自带订阅 OAuth). into_parts 前捕获 orig method/uri/headers; handle_passthrough 剔 Host+Content-Length 保留 Authorization, 流式+非流式 1:1 relay, proxy_log 正常记+token 尽力解析. 不调 convert_request/build_upstream_headers. 6 透传单测+spec claude-code-passthrough.md. 后续: 分组路由 AI 平台拖动排序(独立 task) + endpoints 前端不展示 bug 待查.

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `7b21f33` | (see git log) |
| `3d3593b` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete
