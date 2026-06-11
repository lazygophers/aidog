# ST1: schema DDL 重写 + 独立迁移脚本

- **目标**: 新 schema 落地 + 现有库数据迁移
- **产出**:
  - `src-tauri/migrations/001_init.sql` 全量重写为新 schema（见 design.md DDL）
  - `scripts/migrate_db_v2.py` 独立一次性迁移脚本（不进 app；任务完成删除）
- **验证**: 新库 `sqlite3 .schema` 匹配 design；迁移脚本对 `~/.aidog/aidog.db` 实跑 + 行数校验 + 备份 `aidog.db.bak`
- **资源**: design.md（DDL + 迁移步骤）、现有 001_init.sql
- **依赖**: 无
- **失败处理**: 迁移脚本失败回滚备份；脚本幂等（可重跑）
