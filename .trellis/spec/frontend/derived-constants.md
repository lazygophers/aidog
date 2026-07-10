---
updated: 2026-07-10
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# 前端派生层（常量 → 后端 JSON 派生）

何时被读: 前端硬编码常量（协议列表 / label 映射 / 颜色映射 / 大枚举表）改由后端 JSON 真值源派生时
谁读: trellis-implement sub-agent / main
不遵守的代价: 双真值漂移（前端常量 + 后端 JSON 各自维护，平台扩展时漏改一侧）/ async 转换漏 await 首帧崩 / 竞态 setState（卸载后旧 promise resolve）/ locale 切换不刷新

---

## 单真值源派生 (MUST)

前端平台 / 协议类大枚举常量（`PROTOCOLS` / `PROTOCOL_LABELS` / `PROTOCOL_COLORS` 等 60+ 条表）**禁**手维护第二份 —— **MUST** 从后端 JSON 真值源（`src-tauri/defaults/*.json`，如 `platform-presets.json`）派生：

- **派生层位置**: `src/domains/<域>/defaults.ts`（如 `platforms/defaults.ts`），禁散落多文件
- **单次 RPC 缓存 (MUST)**: module-level `const docPromise = loadDoc()` 单次 invoke，多次调用复用同一 IPC —— 禁每次派生函数内部重发 RPC（N 调用 = N IPC = N×延迟）
- **派生函数签名**: `buildXFromPresets(locale?): Promise<T[]>` / `getXMap(): Promise<Record<K,V>>`，async，返 Promise
- **locale 分桶缓存**: 派生函数内部 `cache[locale]` 按 locale 分桶，禁全局静态单值（locale 切换须刷新派生结果）

## 小常量例外（保留硬编码）

非后端真值源映射的小常量（请求格式协议 5 条 `ENDPOINT_PROTOCOLS` / 路由判定 / UI 固定枚举）**保留**前端硬编码 —— 派生成本 > 收益，且后端无对应真值源无漂移风险。

- 保留条件三全中: ① 后端无对应 JSON 真值源 ② 条目 ≤10 ③ 业务稳定（非平台扩展类，不会随后端 JSON 增长）
- 标注 `Partial<Record<K, string>>`，注释说明「与 `<同集常量>` 同集不同 shape」
- 改薄后 `grep '\b<OLD_CONST>\b' src/` 残留仅合法 fallback（如 endpoint badge 5 请求格式 fallback）+ 注释

## 调用点 async 化范式 (MUST)

派生函数 async 后，所有 caller **MUST** 改 `useEffect + useState` 模式，**禁**保留同步读取（编译报错或拿空值）：

```tsx
const [map, setMap] = useState<Record<K, V>>({});  // 初始空 map/数组，禁 null（防 .map() 崩）
useEffect(() => {
  let cancelled = false;
  buildXFromPresets(i18n.language).then(m => {
    if (!cancelled) setMap(m);
  }).catch(console.error);  // 禁静默丢弃，至少 console.error（见 cross-layer Data Flow）
  return () => { cancelled = true; };
}, [i18n.language]);  // locale key，locale 切换自动重新派生
```

- **首帧 fallback**: useState 初始**空 map / 空数组**（禁 `null`，防下游 `.map()` / `.length` 崩页）；派生数据加载前用空集合渲染，禁 loading spinner 闪烁，禁阻塞首帧
- **cancelled flag (MUST)**: useEffect 清理函数置 `cancelled = true`，防异步竞态 setState（组件卸载后 / locale 切换重渲染后旧 promise resolve 仍 setState → React warn + 数据错位）
- **locale key (MUST)**: useEffect dep 用 `[i18n.language]`，locale 切换自动重新派生；禁空 dep array `[]` 一次性加载（locale 切换不刷新）
- **工具函数默认参数改可选**: 纯工具函数（如 `matchX(input, list?)`）原同步接收常量数组的参数改可选，未传时内部 `await buildXFromPresets()`，保持 caller 简洁（caller 不必每次预加载传入）

## AppContext 预热缓存 (best-effort)

AppContext 顶层调一次 `buildXFromPresets().catch(console.error)` 预热 module-level `docPromise` 单次 RPC 缓存（best-effort，**禁** await 阻塞启动）—— 后续 caller useEffect 命中已 resolve 的缓存，近似同步返回，减少首帧空 map 窗口。

## 验收断言（可复用）

```bash
# 派生层单 RPC 缓存（docPromise module-level 单次 invoke，非函数内）
grep -n 'docPromise' src/domains/*/defaults.ts  # module-level const，非函数内局部

# 调用点对称（cancelled flag + locale key）
grep -rn 'let cancelled = false' src/  # 每个派生 caller 1 处
grep -rn 'i18n.language\]' src/  # useEffect dep 用 locale

# 残留旧大常量（禁手维护第二份大枚举表）
grep -rn '\b<OLD_BIG_CONST>\b' src/  # 仅注释 / 合法小常量保留 / 合法 fallback

# TS 类型跨层对齐（派生后 enum union 须含后端新增变体）
grep -n '<NewVariant>' src/services/api/types/*.ts  # 与 Rust enum serde rename 一致

# 三门禁
yarn build && yarn test && yarn check:i18n
```

## 实例

task 07-10-protocols-frontend-derive（C3）：
- 删 `PROTOCOLS`（81 行 60+ 平台 + cp + mock）+ `PROTOCOL_COLORS`（73 行）
- `PROTOCOL_LABELS` 改薄留 5 请求格式协议条目（与 `ENDPOINT_PROTOCOLS` 同集，Partial 标注），平台 label 走 `buildProtocolsFromPresets` 派生 `labelMap` 单源
- `defaults.ts` 加 `buildProtocolsFromPresets(locale?)` + `getProtocolColorMap()`，内联 `deriveProtocolHosts` 合并旧 `injectProtocolHosts`（禁第二份真值），module-level `docPromise` 单 RPC 缓存
- 13 调用点 async 化（SearchableProtocolSelect / ProtocolLogo / PlatformCard / PlatformListView / PlatformEditForm / Sub2ApiImport / ccswitchMatch / AppContext / usePlatformForm / PlatformPicker / SmartPasteModal / ModelTestPanel），6 文件对称（useState 空初始 + cancelled flag + locale key）
- AppContext `buildProtocolsFromPresets().catch(...)` 预热 docPromise
- `ccswitchMatch.ts matchCcProvider(provider, protocols?)` 默认参数改可选，未传时内部 await
- types/part1.ts Protocol union 补 3 cp 变体（kimi_coding / qianfan_coding / xiaomi_mimo_coding）对齐 Rust enum
- 测试 mock 补 `keywords` / `name` 字段对齐 JSON 真值（派生后 mock 须与 JSON 真值一致，否则 keywords 误匹配 base_url 子串）

## Cross-reference

- 真值源: `src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖，见 project CLAUDE.md）
- 跨层 enum lowercase / 集合非 null: [Cross-Layer Rules](../guides/cross-layer-rules.md) Format Contracts
- locale 标签三层一致: [Locale Tag Cross-Layer](./locale-tag-cross-layer.md)
- 数据流单向 / useEffect+useState / catch 禁静默: [Cross-Layer Rules](../guides/cross-layer-rules.md) Data Flow
