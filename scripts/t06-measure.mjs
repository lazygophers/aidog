// 临时测量脚本（票 06）：统计 tauri_command! 命令里 AppHandle / State 参数分布。
import { readFileSync } from "fs";
import { execSync } from "child_process";
const files = execSync("find src-tauri -name '*.rs' -not -path '*/target/*'", {
  encoding: "utf8",
})
  .trim()
  .split("\n");
let cmds = [];
for (const f of files) {
  const s = readFileSync(f, "utf8");
  const re = /tauri_command!\s*\{/g;
  let m;
  while ((m = re.exec(s))) {
    let i = re.lastIndex - 1,
      d = 0,
      end = -1;
    for (let j = i; j < s.length; j++) {
      if (s[j] === "{") d++;
      else if (s[j] === "}") {
        d--;
        if (d === 0) {
          end = j;
          break;
        }
      }
    }
    const blk = s.slice(i, end + 1);
    const sig = blk.match(/pub (?:async )?fn\s+(\w+)\s*\(([\s\S]*?)\)\s*->/);
    if (!sig) continue;
    const [, name, args] = sig;
    cmds.push({
      file: f,
      name,
      hasA: /AppHandle/.test(args),
      hasS: /State</.test(args),
    });
  }
}
console.log("commands parsed:", cmds.length);
console.log("AppHandle param:", cmds.filter((c) => c.hasA).length);
console.log("State<> param:", cmds.filter((c) => c.hasS).length);
console.log("both:", cmds.filter((c) => c.hasA && c.hasS).length);
console.log("neither:", cmds.filter((c) => !c.hasA && !c.hasS).length);
const byfile = {};
for (const c of cmds)
  if (c.hasA || c.hasS)
    (byfile[c.file] ??= []).push(
      c.name + (c.hasA ? "[A]" : "") + (c.hasS ? "[S]" : ""),
    );
for (const k of Object.keys(byfile).sort())
  console.log(" ", k, byfile[k].length);
