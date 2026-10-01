#!/usr/bin/env bash
# 经管理面 /rpc 灌配置（走命令层 → 自动失效 groups 缓存，无需重启）。需 start.sh 以 UI=1 起。
# 分组 group_key：perf-conv（anthropic 入 → openai 上游，协议转换）/ perf-pass（anthropic → anthropic 同协议透传）
# LOG_BODIES=1（默认，镜像用户实测：log_user_request=log_upstream_request=true）/ 0 = 只留元数据
set -euo pipefail; source "$(dirname "$0")/env.sh"
rpc() { curl -sf -X POST "http://127.0.0.1:$MGMT_PORT/rpc/$1" -H 'content-type: application/json' -d "$2"; }
if rpc group_list '{}' | jq -e 'map(select(.group_key=="perf-conv")) | length > 0' >/dev/null; then
  echo "already seeded ($ISO)"; exit 0; fi
lb=$([ "${LOG_BODIES:-1}" = 1 ] && echo true || echo false)
rpc proxy_log_settings_set "{\"settings\":{\"enabled\":true,\"log_user_request\":$lb,\"log_upstream_request\":$lb}}" >/dev/null
mk_platform() { # name protocol base_url model → id
  rpc platform_create "$(jq -nc --arg n "$1" --arg p "$2" --arg u "$3" --arg m "$4" '{input:{
    name:$n, platform_type:$p, base_url:$u, api_key:"sk-mock-perf",
    models:{default:$m, sonnet:$m, opus:$m, haiku:$m},
    endpoints:[{protocol:$p, base_url:$u, client_type:"default"}],
    auto_group:false, quota_source:"manual"}}')" | jq -r .id
}
mk_group() { # key platform_id
  gid=$(rpc group_create "$(jq -nc --arg k "$1" '{input:{name:$k, group_key:$k, routing_mode:"failover", source_protocol:"anthropic"}}')" | jq -r .id)
  rpc group_set_platforms "$(jq -nc --argjson g "$gid" --argjson p "$2" '{input:{group_id:$g, platforms:[{platform_id:$p, priority:1, weight:1}]}}')" >/dev/null
  echo "group $1 id=$gid → platform $2"
}
p_oai=$(mk_platform perf-openai openai "http://127.0.0.1:$MOCK_PORT/v1" mock-gpt)
p_ant=$(mk_platform perf-anthropic anthropic "http://127.0.0.1:$MOCK_PORT" mock-claude)
mk_group perf-conv "$p_oai"
mk_group perf-pass "$p_ant"
