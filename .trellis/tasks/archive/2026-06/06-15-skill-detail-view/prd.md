# PRD: skill-detail-view

## 背景
已装 skill 列表行（Skills.tsx L737 skill.name）目前纯展示，无详情入口。用户需求：点击 skill 名 → 查看详情（SKILL.md 渲染 + 关联文件浏览），**只读禁编辑**。

## 现状
- SkillInfo.installed_path = skill 目录绝对路径（`~/.agents/skills/<name>/` 或 project 级）
- skill 目录 1-175 文件不等（SKILL.md 核心 + README/references/assets/LICENSE 等）
- 项目**无 markdown 渲染库**
- 后端无通用读文件/列目录 command
- 安全 precedent: [[pathbuf-starts-with-traversal]] — PathBuf::starts_with 词法比较，须 canonicalize + 显式拒 `..`/绝对路径

## 决策
1. **详情 UI = 大 modal（createPortal）**: 双栏 — 左文件树，右内容查看器。max 90vw × 85vh。复用现有 modal 模式（fixed + overlay + ESC/点遮罩关闭）。
2. **markdown 渲染**: 加 `react-markdown` + `remark-gfm`（标准库，~30KB gzip）。SKILL.md + README*.md 走渲染；其他文件 `<pre>` 纯文本。**只读**，无编辑。
3. **触发点**: skill.name 改为可点击 button（保留 agent toggle / 卸载按钮不变，避免误触）。
4. **后端读文件**: 新增 command（非用 plugin-fs），统一做路径遍历防护 + 二进制检测 + 大小上限。

## 改动清单

### 1. 后端：新增 skill 详情 commands
**文件**: `src-tauri/src/gateway/skills.rs` + `src-tauri/src/lib.rs`

`skills.rs`:
```rust
pub struct SkillFile {
    pub rel_path: String,   // 相对 skill 根的路径（/ 分隔）
    pub size: u64,
    pub is_text: bool,      // 启发式: 首块无 NUL 字节 → text
}

pub struct SkillDetail {
    pub root: String,       // canonicalized skill 根
    pub skill_name: String,
    pub files: Vec<SkillFile>,  // SKILL.md 置首，其余字母序
}

pub struct SkillFileContent {
    pub content: Option<String>,  // None = 二进制/读失败
    pub truncated: bool,          // 超 MAX_READ_BYTES (512KB) 截断
    pub size: u64,
}

/// 列 skill 目录文件树（递归，相对路径）。canonicalize + 校验 installed_path 存在。
pub fn detail(installed_path: &str) -> Result<SkillDetail, String>

/// 读单文件。rel 须在 skill 根内（canonicalize 后 starts_with），拒 `..`/绝对。
pub fn read_file(installed_path: &str, rel: &str) -> Result<SkillFileContent, String>
```

安全（hard）:
- `installed_path` canonicalize；`rel` 标准化：拒含 `..` 段 / 以 `/` 或盘符开头
- 拼接后再 canonicalize，断言 `starts_with(skill_root_canonicalized)`
- 二进制检测: 读首 8KB，含 NUL → is_text=false
- 大小上限: 512KB 截断（truncated=true）
- 跳过隐藏文件（`.git` 等）？保留 `.env.example` 类 → 只跳 `.git/` 目录，其他 dotfile 保留

`lib.rs`:
```rust
#[tauri::command] async fn skill_detail(installed_path: String) -> Result<SkillDetail, String>
#[tauri::command] async fn skill_read_file(installed_path: String, rel: String) -> Result<SkillFileContent, String>
```
注册 invoke_handler。

### 2. 前端：api 层 + 详情组件
**文件**: `src/services/api.ts` + `src/pages/SkillDetailView.tsx`

api.ts: `skillsApi.detail(path)` / `skillsApi.readFile(path, rel)` + 类型 SkillFile/SkillDetail/SkillFileContent。

SkillDetailView.tsx（新）:
- Props: `{ skill: SkillInfo, onClose: () => void }`
- 加载: `detail(skill.installed_path)` → 文件树
- 默认选 SKILL.md（若有）→ `readFile` 渲染
- 左栏: 文件列表（rel_path，SKILL.md 高亮置顶），点击切换
- 右栏: 
  - `.md` 文件 → `<ReactMarkdown remarkPlugins={[remarkGfm]}>`
  - 其他文本 → `<pre>` monospace
  - 二进制 → "二进制文件，无法预览"
  - 加载中 → spinner
- 头部: skill 名 + source + enabled agents 徽章 + 关闭按钮
- modal overlay（点击遮罩/ESC 关闭），内容 `onClick stopPropagation`

### 3. Skills.tsx 接线
- 新 state `detailTarget: SkillInfo | null`
- skill.name `<div>` → `<button onClick={() => setDetailTarget(skill)}>`（样式保持，加 hover 下划线/pointer）
- `{detailTarget && createPortal(<SkillDetailView skill={detailTarget} onClose={...} />, document.body)}`

### 4. 依赖
`package.json`: 加 `react-markdown` + `remark-gfm`（最新稳定版）

### 5. i18n（8 语言）
新增 key（`skills.detail.*`）:
- `skills.detail.title` 详情 / Details
- `skills.detail.files` 文件 / Files
- `skills.detail.binary` 二进制文件，无法预览 / Binary file, cannot preview
- `skills.detail.loadFailed` 加载失败 / Failed to load
- `skills.detail.readFailed` 读取失败 / Failed to read
- `skills.detail.truncated` 文件过大，已截断 / File too large, truncated
- `skills.detail.noSkillMd` 无 SKILL.md / No SKILL.md
- `skills.detail.empty` （空 skill 目录）/ Empty skill directory
- `skills.detail.viewDetail` 查看详情 / View details

## 验证
- `cd src-tauri && cargo build && cargo clippy`（0 warning）
- `cd src-tauri && cargo test`（skills 现有 46 + 新增 detail/read_file 路径遍历防护单测）
- `yarn build`（tsc + vite）
- `node scripts/check-i18n.mjs` 零缺失
- 手动: 点 skill 名 → modal 开 → SKILL.md 渲染 → 点其他文件切换 → 二进制提示 → 关闭
- 安全单测: `rel="../etc/passwd"` / `rel="/etc/passwd"` / symlink 逃逸 均拒

## 非目标
- 不支持编辑（硬约束）
- 不支持文件下载/导出
- 不做语法高亮（`<pre>` 足够，避免引入 highlight.js 重依赖）
- 不支持搜索文件内容
- 不展示图片预览（assets/ 下列出但不渲染图片，只标记二进制）

## 风险
- react-markdown 版本兼容 React 19 → 用最新（已支持 React 19）
- 大 skill（175 文件 huashu-design）文件树加载 → 递归列目录快（<50ms），可接受
- SKILL.md 内 frontmatter → react-markdown 默认当表格文本渲染，可接受（不引入 frontmatter parser）
- 路径遍历 → canonicalize + starts_with 双校验 + 单测覆盖
