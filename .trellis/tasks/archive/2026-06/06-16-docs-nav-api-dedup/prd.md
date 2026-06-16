# docs 顶部栏 API 接口条目修复

## 背景
- `docs/docs/<lang>/_nav.json` 控制顶部导航栏。
- zh/en/fr/de/ja：API 条目重复 2 次（6 条 → 应 5 条）。
- ar/ru：API 条目 1 次（5 条，正确）。
- es：完全缺失 API 条目（4 条，应 5 条）。

## 改动
- zh/en/fr/de/ja：删除重复的最后一条 API 条目。
- es：补一条 API 条目 `{"text":"Referencia de API","link":"/es/api/api-reference","activeMatch":"/es/api"}`。

## 验证
- 8 语言 `_nav.json` 均恰好 5 条，API 条目唯一。
