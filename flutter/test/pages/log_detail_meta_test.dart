/// 日志详情元信息区的三项回归（2026-09-22）。
///
/// 三条都出自平台页/日志页重审，前两条是同一类问题：
///   1. `is_stream` **模型层没接**（`types/manual.ts:649` 后端一直在发）——
///      与 `attempts` / `MiddlewareRule.failed` 同类，逐元素比对表照不出来。
///   2. `upstream_status_code` 早就解析进来了，详情区就是没这一项。
///   3. 「分组」印的是 `group_key` 原文 —— 那串正好是该分组的 API Key，
///      分组卡已经因为这个理由不印它了，详情面板这处还留着。
library;

import 'package:aidog_flutter/pages.dart';
import 'package:flutter_test/flutter_test.dart';


Map<String, Object?> _detail({
  bool isStream = true,
  int upstreamStatus = 200,
}) => {
  'id': 'a1',
  'group_key': 'gk10',
  'model': 'm',
  'actual_model': 'am',
  'status_code': 200,
  'upstream_status_code': upstreamStatus,
  'is_stream': isStream,
  'duration_ms': 12,
  'request_headers': '{}',
  'request_body': '{}',
  'response_body': '{}',
};

void main() {
  test('ProxyLogDetail 解析 is_stream（原先模型里压根没这个字段）', () {
    expect(ProxyLogDetail.fromJson(_detail(isStream: true)).isStream, isTrue);
    expect(ProxyLogDetail.fromJson(_detail(isStream: false)).isStream, isFalse);
    // 后端没发时按非流式，不要把旧数据全判成流式。
    final noField = Map<String, Object?>.from(_detail())..remove('is_stream');
    expect(ProxyLogDetail.fromJson(noField).isStream, isFalse);
  });

  test('上游状态码 0 = 没捕获到，不是一个真的 HTTP 码', () {
    expect(ProxyLogDetail.fromJson(_detail(upstreamStatus: 0)).upstreamStatusCode, 0);
    expect(ProxyLogDetail.fromJson(_detail(upstreamStatus: 502)).upstreamStatusCode, 502);
  });
}
