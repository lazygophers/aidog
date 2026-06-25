# 平台过期时间字段与到期自动禁用

## 目标

平台加可选过期时间 (expires_at)。到期后平台自动从路由候选排除 (等效禁用); 清理失效平台时一并清过期平台; 智能粘贴识别文案中的过期时间 (如有)。

## 数据模型

### Platform 表新增列
- `expires_at INTEGER NOT NULL DEFAULT 0` (毫秒 unix 时间戳, 0 = 永不过期)
- Migration (schema_late.rs 新增 migration)

### Platform struct (models/platform.rs)
- `pub expires_at: i64` (#[serde(default)], 0 = 无过期)

### CreatePlatform / UpdatePlatform
- `expires_at: Option<i64>` (None = 不动; Some(0) = 清空; Some(t) = 设)

## 到期排除 (不新增 status 变体)

`candidate_state` (router/mod.rs:48) 入口加检查:
```rust
pub(crate) fn candidate_state(platform: &Platform, now_ms: i64) -> Option<bool> {
    // 过期平台直接排除 (等效自动禁用, 不改 status 枚举)
    if platform.expires_at > 0 && now_ms >= platform.expires_at {
        return None;
    }
    match platform.status { ... }
}
```
- 不新增 PlatformStatus::Expired (避免三态→四态连锁: 恢复逻辑 / 前端三态切换 / serde)
- 过期是独立维度, 与 status 正交: 过期平台即使 status=Enabled 也被排除
- 用户改 expires_at (清空或延后) 即恢复, 无需退避试探

## UI 展示

- 平台卡片: `expires_at > 0 && now >= expires_at` → 展示"已过期"标记 (红色 badge, 类似 auto_disabled 展示)
- 平台卡片: `expires_at > 0 && 未过期` → 展示"MM-DD HH:MM 到期"小字 (临近过期高亮)
- 编辑表单: datetime 输入 + 清空按钮 (非必填)

## 清理失效平台

`purge_auto_disabled_platforms` (platform_lifecycle.rs:90) 扩范围:
- 清 `(status=auto_disabled OR expires_at>0 AND expires_at<now)` 的平台
- 命令 `platform_purge_disabled` / 前端 `purgeDisabled` 文案对齐

## 智能粘贴识别

`parsePlatformPaste` (platformPaste.ts) 加 `expiresAt` 提取:
- 模式: 文案含 "过期/到期/exp/expire/有效期" 附近的时间
- 解析: "06-28 23:59" (当年), "2026-06-28", "06-28", 相对时间
- 社区帖子常见 "即将过期 06-28 23:59"
- 返回毫秒时间戳; 无法确定时省略
- 前端 SmartPasteModal 回填表单 expires_at

## 验收

1. 平台设 expires_at = 过去时间 → 路由不选该平台 (proxy 请求跳过)
2. 平台设 expires_at = 未来时间 → 正常使用, 卡片显示到期日
3. 清空 expires_at → 平台恢复
4. purge 清理 auto_disabled + 过期平台
5. 粘贴 "即将过期 06-28 23:59 ... token" → 表单回填过期时间
6. cargo clippy/test 全绿; yarn build; i18n 零缺失
7. migration 向后兼容 (旧行 expires_at=0)

## 已定决策 (用户裁定)

1. **状态表示**: 独立字段 `expires_at` + `candidate_state` 路由排除 (不新增 PlatformStatus 变体, 不复用 auto_disabled)。
2. **触发**: 懒加载路由排除 (无后台调度器, 与 auto_disabled 同模式)。
3. **智能粘贴**: 宽松模式 — 文案中任何形如 `MM-DD HH:MM` / `YYYY-MM-DD` / `MM-DD` 的时间都尝试解析。
   - 回退保护: 解析出的时间若早于 `now - 7d` (明显历史日期) 视为无效跳过, 不回填。
   - 多日期候选: 优先取靠近"过期/到期/exp/有效期"语义词的; 无语义词时取第一个有效未来日期。
   - 默认补全年份为当年; 若解析出的日期已过 (当年该日 < 今天), 推到次年。

## 实施拆分

见 `implement.md`。

## 回归 bug (2026-06-25 用户报，check 阶段追加)

**现象**: 手动填表单新增平台（非智能粘贴/复制），未设过期时间，但保存后某处显示了过期时间（"自动填入"）。

**候选根因** (bug-hunt agent a54e196f 只读定位中):
1. handleSave 传 `expires_at` snake_case key (Platforms.tsx:2220,2234) —— 违 memory [[tauri-invoke-param-camelcase]]（invoke 参数须 camelCase），致后端 Option 恒 None
2. datetime-local input 在 Tauri WKWebView value="" 渲染怪异
3. React state 时序 / resetForm 未触发
4. 后端 row 映射列序错位
5. 复制/编辑残留 state

**顺带修**: smart paste `extractExpiryAt` (platformPaste.ts:416) 收紧 —— 仅当日期附近有过期语义词才识别（用户已认可 Q2）。

**实施**: bug-hunt 定位完 → 派 implement agent 修（手动填表单根因 + invoke camelCase 防御 + smart paste 收紧）。

## UI polish: datetime-local 主题适配 (2026-06-25 用户报)

**现象**: 过期时间 `<input type="datetime-local">` (Platforms.tsx:3156) 未适配多主题 —— style 仅 `{flex:1, minWidth:200}`，走浏览器原生默认（白底黑字），在 dark/彩色主题下突兀。

**根因**: 缺 `colorScheme` CSS 属性（控原生日历弹出层明暗）+ input 本体未走主题 CSS 变量（color/background/border）。

**方案** (polish agent 实现):
1. input style 加 `colorScheme: mode`（light/dark，控 WKWebView 原生日历弹出层 + 输入框明暗）—— 从 useTheme 拿当前 mode
2. input 本体 color/background/border 走 CSS 变量（参照同表单其他 input 的 className/style 模式，如 `var(--text-primary)` / `var(--bg-input)` / `var(--border)`）
3. 主题 mode 注入: documentElement `data-theme-*` 属性（themes/index.ts:134-136 + types.ts:52-72），useTheme hook 暴露 mode

**并入 check polish**: 与 bug 修复 + smart paste 收紧一起做。
