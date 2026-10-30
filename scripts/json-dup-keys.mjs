// 找 JSON 文本里同一对象内的重复键（JSON.parse 静默取后者，丢前者）。返回 [{key, line, firstLine}]。
export function findDuplicateKeys(text) {
  const dups = [];
  const stack = []; // 每层对象: Map<key, firstLine>；数组层记 null
  let line = 1;
  let expectKey = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === "\n") line++;
    else if (c === "{") { stack.push(new Map()); expectKey = true; }
    else if (c === "[") { stack.push(null); expectKey = false; }
    else if (c === "}" || c === "]") { stack.pop(); expectKey = false; }
    else if (c === ",") expectKey = stack[stack.length - 1] instanceof Map;
    else if (c === '"') {
      const start = i;
      for (i++; text[i] !== '"'; i++) if (text[i] === "\\") i++;
      if (expectKey) {
        const key = JSON.parse(text.slice(start, i + 1));
        const scope = stack[stack.length - 1];
        if (scope.has(key)) dups.push({ key, line, firstLine: scope.get(key) });
        else scope.set(key, line);
        expectKey = false;
      }
    }
  }
  return dups;
}
