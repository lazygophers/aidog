/**
 * 「回客户端正文」取值合并：user_response_body 优先，为空回退 response_body，都空返回 null。
 * 占位文案由调用方按场景给（剪贴板保英文、UI 走 i18n），不在函数内置。
 * 票 06：流式哨兵字面量已废，空串 = 流式中 / 日志开关关闭 / 内容未捕获。
 */
export function pickUserResponseBody(
  userResponseBody: string | null | undefined,
  responseBody: string | null | undefined,
): string | null {
  return userResponseBody || responseBody || null;
}
