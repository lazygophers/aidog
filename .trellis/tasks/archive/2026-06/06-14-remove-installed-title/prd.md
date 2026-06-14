# PRD: Skills 页布局整合 + dark mode 适配

## 背景
Skills 页当前 4 处问题:
1. 顶部 scope 筛选卡 与 总计统计卡 分离,信息割裂
2. "已安装" h3 标题冗余 (列表本身已表意)
3. scope 筛选未右对齐,视觉重心偏左
4. **dark mode 字体/背景色 bug**: Skills.tsx:332 用 `var(--accent-soft, rgba(255,255,255,0.10))` —— 变量名错 (主题实际定义是 `--accent-subtle`, 见 liquidGlass.ts:25/51),fallback `rgba(255,255,255,0.10)` 永久生效,dark mode 下 enabled toggle 按钮背景是错误白色块。

## 目标 (单交付, main worktree 内直接写)
1. **合并 scope 筛选 + 统计卡** 为单个 glass-elevated card: 左侧统计 (总数 + 每 agent 启用数), 右侧 scope 筛选 (右对齐)
2. **删 "已安装" h3 标题** (line 284)
3. **修 dark mode bug**: `--accent-soft` → `--accent-subtle`, 清除误导性 fallback
4. scope 筛 select/project path 右对齐

## 范围
- `src/pages/Skills.tsx`: 单文件改

## 验证
- `yarn build` exit 0
- light + dark 主题下 enabled/disabled toggle button 颜色正确 (enabled=accent-subtle 背景, 非 fallback 白)
- 合并卡布局: 左统计 右筛选, 无视觉断裂

## 不做
- 不改主题文件 (变量已正确定义, 是引用方错)
- 不加 i18n (无新文案)
- 不动后端
