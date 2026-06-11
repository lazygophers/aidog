# Design: 修复流式 proxy_log token=0

## 改动（proxy.rs 流式分支）
- 流式 setup 处：clone log（含 id/group_name/model 等已填字段）+ state + log_settings 进 stream 闭包（与 spawn_estimate 同款 clone）
- `[DONE]` 分支（:574 区，est_fired.swap 守卫内）：
  ```
  let mut final_log = log_for_done.clone();
  final_log.input_tokens = est_in.load(Relaxed);
  final_log.output_tokens = est_out.load(Relaxed);
  final_log.cache_tokens = est_cache.load(Relaxed);
  final_log.status_code = ...; // 流式成功 200（已知）
  upsert_log(&state_for_done, &final_log, &log_settings_for_done);
  ```
  （upsert_log 是 INSERT OR REPLACE，同 log.id 覆盖；与 spawn_estimate 共用 est_fired 守卫，保证 token upsert + 预估各一次）
- 提前的 :505 upsert_log 保留（记录请求元数据/响应头等），[DONE] 仅覆盖 token（+ 必要状态）
- 注意：clone 所有权（log/state/settings move 进 'static 闭包）；est_fired 已有

## 验证
- cargo build + test；流式请求 proxy_log token 非 0；非流式分支不动；[DONE] upsert 一次
