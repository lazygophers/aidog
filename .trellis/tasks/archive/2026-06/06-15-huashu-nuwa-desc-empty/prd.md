# PRD: huashu-nuwa desc 空不展示

## 现象
huashu-nuwa 扫描已有 perspective skill 展示候选时，description 为空的 skill 仍被列出，模型自由输出 `name | desc` 行内格式导致残留 `name |`（孤立分隔符）。

## 根因
SKILL.md L111「来源A：本地已有Skill」段：扫描 `.claude/skills/*-perspective/` 读 description 匹配，但未规定：
1. description 为空的 skill 如何处理（应跳过）
2. 展示格式（模型自由发挥 `name | desc` 行内，desc 空残留 `|`）

## 修复
`~/.agents/skills/huashu-nuwa/SKILL.md` L111「来源A」段补两条规则：
1. **跳过空 desc**：description 为空的 skill 不纳入候选（不展示，避免 `name |` 残留）
2. **禁行内 | 格式**：展示已有 skill 必须用下方候选格式（`### 候选N` + `**核心镜片**`），不要用 `name | desc` 行内格式

## 产出
`/Users/luoxin/.agents/skills/huashu-nuwa/SKILL.md` L111 段补规则文本。

## 验证
- 文本审查：规则清晰，与候选格式（L116-124）一致
- 无 build/test（纯文档 skill）

## 范围（不做）
- 不改候选展示格式本身
- 不改其他段

## 备注
huashu-nuwa 非 git 仓库（`~/.agents/` 非 git），文件直接改，无 commit。aidog task 仅跟踪。
