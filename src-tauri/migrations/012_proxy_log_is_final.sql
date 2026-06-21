-- Migration 012 (SQL file) / inline migration 033: proxy_log 终态标记列 is_final。
-- 一个请求生命周期内 upsert_log 被调用 40+ 次（INSERT + 多次 UPDATE + 流式 flush），
-- 中间节点 status_code=0 或 response_body=='[stream]' 占位。is_final=1 仅在首个真实终态
-- （有 HTTP 状态、非流式占位）写入时置位，标记该行已是请求的最终结果。
-- 旧库走 db.rs init_tables 内联 ALTER（ALTER 无 IF NOT EXISTS，重复列报错被忽略），
-- 此文件供新装库 / 文档参考。
ALTER TABLE proxy_log ADD COLUMN is_final INTEGER NOT NULL DEFAULT 0;
