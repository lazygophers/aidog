# 平台编辑/创建表单: 唯一分组时设 level_priority

## 背景

`level_priority` 是 **group×platform 关联级** 字段 (0-10, 10=最高优先, 仅该 group 内有效), 已在 Groups 页 PlatformCard 就地编辑 (上下按钮, `group.levelPriority*` i18n, `group_platform_set_level_priority` command)。

平台全局表单 (Platforms 页) 语义上不该设它 — 因平台可挂多 group, 每 group 各自一个值。**但当平台关联唯一分组时**, 在平台表单直接设值合理 (避免用户切到 Groups 页二次操作)。

## 目标

Platforms 页平台编辑/创建表单: 当关联**恰好一个分组**时, 提供 level_priority 输入; 多分组或零分组时不提供。

## 唯一分组判定

| 场景 | autoGroup | joinGroupIds | 总分组数 | 显示控件 |
|---|---|---|---|---|
| 创建, 默认建组 | true | [] | 1 (默认组) | ✅ |
| 创建, 加入单个已有组 | false | [x] | 1 | ✅ |
| 创建, 加入多个已有组 | any | [a,b] | ≥2 | ❌ 隐藏 |
| 创建, 不建组也不加入 | false | [] | 0 | ❌ 隐藏 |
| 创建, 建组 + 加入已有 | true | [x] | 2 | ❌ 隐藏 |
| 编辑, 已挂 1 group | — | — | 1 | ✅ (回填当前值) |
| 编辑, 已挂 ≥2 group | — | — | ≥2 | ❌ 隐藏 (提示去 Groups 页) |

`唯一分组 ID` = 默认组(创建后回查) 或 joinGroupIds[0]。

## 功能规格

### 创建路径
1. 表单显示 level_priority 控件 (0-10, 默认 5), 仅当上述判定 == 1 时。
2. autoGroup 建的默认组: **后端 `CreatePlatform` 加 `default_level_priority: Option<i32>` 字段**, `create_auto_group_for` → `set_group_platforms` 时用该值 (None→默认 5)。create 返回后前端无需回查。
3. joinGroupIds[0] (不建默认组, 仅加入单个已有组): create 返回 Platform.id 后, 前端调 `group_platform_set_level_priority(joinGroupIds[0], platformId, levelPriority)`。

### 编辑路径
1. 进入编辑: 从 groupDetails 查该平台所属 group 列表 (groupId + level_priority)。
2. 若唯一 group: 回填该关联的 level_priority (默认 5 若无)。
3. 若 ≥2 group: **直接隐藏控件** (不提示)。
4. handleSave update 成功后, 若唯一 group 且值变更: 调 `group_platform_set_level_priority(groupId, platformId, levelPriority)`。
   - 编辑改归属 (joinGroupIds 变更): update 内已全量同步成员关系; 若新归属唯一, update 后对该唯一 group 调 set_level_priority。

### UI 控件
- 复用 Groups 页同款: 数字输入 + 上下按钮 + hint (数值越大优先级越高, 10=最高, 仅本分组生效)。
- 位置: 表单内分组归属选项 (autoGroup/joinGroupIds) 附近。
- i18n: 复用现有 `group.levelPriority*` key, 不新增。

## 不做

- 不新增 Platform.priority 列 (Platform 表无此字段, 调度优先级只在关联级)。
- 不改 router/group_platform.priority (拖拽连续序号) 语义。
- 不在多分组场景强行批量改 (语义复杂, 明确不做)。

## 验收

1. 创建平台 (autoGroup 默认勾), 表单出现 level_priority, 设值保存 → Groups 页该平台卡显示设的值。
2. 创建平台加入 2 个已有组, 表单无 level_priority 控件。
3. 编辑只挂 1 group 的平台, 表单回填当前 level_priority, 改值保存生效。
4. 编辑挂 2 group 的平台, 表单无控件, 展示多分组提示。
5. cargo clippy / cargo test 全绿; yarn build 通过; i18n 无新增裸 key。

## 待确认 (评审)

已确认 (见下决策):
- autoGroup 默认组 level_priority → 后端 CreatePlatform 加 default_level_priority 字段。
- 多分组编辑 → 直接隐藏控件。
