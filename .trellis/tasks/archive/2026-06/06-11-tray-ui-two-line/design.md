# Design: 托盘两行 + 隐藏 logo

## tray_quota_text 改（lib.rs:1228）
返回两行文字（含平台名）：
```
let name = platform.name;
let second = if platform.tray_display == "coding" {
    let tier = EstCodingPlan::from_json(&platform.est_coding_plan).tiers.first()?;
    format!("剩 {:.0}%", (100.0 - tier.est_utilization).max(0.0))
} else {
    format!("{:.2}", platform.est_balance_remaining)
};
Some(format!("{name}\n{second}"))   // 两行，无 emoji（用户要求纯文字）
```
- menu item（:1265 tray_quota）可保留展示 second（或两行），下拉详情

## refresh_tray_menu 改（:1279）
```
let tray = ...;
tray.set_menu(...)?;
#[cfg(target_os = "macos")]
{
    match tray_quota_text(app) {
        Some(text) => {
            tray.set_icon(None)?;          // 有值隐藏 logo
            tray.set_title(Some(&text))?;  // 两行（\n）
        }
        None => {
            tray.set_icon(Some(app.default_window_icon().cloned().unwrap()))?; // 恢复 logo
            tray.set_title(None)?;
        }
    }
}
```

## \n 两行验证 + 降级
- macOS NSStatusItem set_title 传 `\n` —— 实施 agent 验证 Tauri 2.0 是否渲染两行：
  - 若渲染两行 → 用 `{name}\n{second}`
  - 若仅单行/显示异常 → 降级 `{name} {second}`（单行紧凑），prd 决策已授权
- agent 查 Tauri tray set_title 文档/源码确认 \n 行为；GUI 实际渲染留用户验，代码逻辑两行优先 + 降级注释

## 不改
- 非 macOS：仅 menu item（无 set_title/icon 切换）
- est 数据源、前端、schema

## 验证
- cargo build 0；逻辑：有值隐 icon+两行(降级单行)、无值恢复 icon+清 title；coding 剩余%/balance 总余额
