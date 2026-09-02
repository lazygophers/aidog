// 票 06 零差集证明（二）：扫全仓 `tauri_command! {` 块，抽出块内 `pub [async] fn <name>`
// 的函数名集合（= 实际生成 `#[tauri::command]` 的名字），排序后逐行输出。
//
// 与 t06-handler-names.mjs 的输出互为交叉验证：注册表（前端 invoke 的真值源）与
// 宏产出的命令名两侧都不能因为本次重构而变化。
import { readFileSync } from "fs";
import { execSync } from "child_process";

const files = execSync("find src-tauri -name '*.rs' -not -path '*/target/*'", {
  encoding: "utf8",
})
  .trim()
  .split("\n");

// 票 07：测试模块里也会用 `tauri_command!` 定义夹具命令（宏的 HTTP 展开要按分支取样），
// 它们不进 `generate_handler!`，比对时必须排除。判据不是文件名（`aidog_cli_proxy/src/
// test_cmd.rs` 是**生产**命令 `cli_proxy_test` 的家），而是「被 `#[cfg(test)] #[path=..]`
// 挂进来的文件」——只有这种文件不参与编译产物。
const testOnly = new Set();
for (const f of files) {
  const src = readFileSync(f, "utf8");
  const re = /#\[cfg\(test\)\]\s*#\[path\s*=\s*"([^"]+)"\]/g;
  let m;
  while ((m = re.exec(src))) {
    testOnly.add(f.replace(/[^/]+$/, m[1]));
  }
}

const names = [];
for (const f of files) {
  if (testOnly.has(f)) continue;
  const src = readFileSync(f, "utf8");
  const re = /tauri_command!\s*\{/g;
  let m;
  while ((m = re.exec(src))) {
    // 从 `{` 起做花括号配平，拿到完整宏体
    const open = re.lastIndex - 1;
    let depth = 0;
    let end = -1;
    for (let j = open; j < src.length; j++) {
      if (src[j] === "{") depth++;
      else if (src[j] === "}") {
        depth--;
        if (depth === 0) {
          end = j;
          break;
        }
      }
    }
    if (end < 0) throw new Error(`unbalanced tauri_command! block in ${f}`);
    const blk = src.slice(open, end + 1);
    const sig = blk.match(/pub\s+(?:async\s+)?fn\s+(\w+)\s*\(/);
    if (sig) names.push(sig[1]);
  }
}

const uniq = [...new Set(names)].sort();
console.error(`# tauri_command! blocks: ${names.length}, unique names: ${uniq.length}`);
console.log(uniq.join("\n"));
