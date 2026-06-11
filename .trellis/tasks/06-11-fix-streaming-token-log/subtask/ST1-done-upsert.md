# ST1: [DONE] 闭包 upsert_log 更新 token

- **目标**: 流式日志 token 在流消费完后正确写入
- **产出** (proxy.rs 流式分支):
  - clone log/state/log_settings 进 stream 闭包（同 spawn_estimate clone 款）
  - [DONE] 分支（est_fired.swap 守卫内）：final_log = log clone，填 est_in/est_out/est_cache.load 最终 token + status，upsert_log（INSERT OR REPLACE 覆盖）
  - 提前 :505 upsert_log 保留（元数据），[DONE] 覆盖 token
- **验证**: cargo build 0
- **资源**: design.md、proxy.rs:531(tokens_acc)/:558(stream)/:574([DONE])/:505(早 upsert)/:100(spawn_estimate clone 款)
- **依赖**: 无
- **失败处理**: 闭包所有权/move → clone 'static；est_fired 复用守卫
